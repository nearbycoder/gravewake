use crate::anatomy::{self, Anatomy, BodyEvent, Part, Pose};
use crate::controls::{Action, Bindings, Device, Trigger};
use crate::world_layout::{self, ENEMY_RADIUS, PLAYER_RADIUS};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// A well-mixed seed from the clock and a per-process counter (SplitMix64).
fn fresh_seed() -> u64 {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut z =
        nanos ^ (u64::from(std::process::id()) << 32) ^ count.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

const SAVE_FILES: [&str; 3] = ["run.json", "graphics.json", "performance.json"];

// Where saves live, plus older locations to import from, in priority order.
// macOS keeps its Application Support folder. Linux and other Unix systems use
// the XDG data directory; builds before Linux support wrote to the macOS path.
pub(crate) fn save_dirs(
    var: impl Fn(&str) -> Option<std::ffi::OsString>,
) -> (PathBuf, Vec<PathBuf>) {
    let home = var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let mac_support = home.join("Library/Application Support");
    if cfg!(target_os = "macos") {
        (
            mac_support.join("Gravewake"),
            vec![mac_support.join("Dark Veil")],
        )
    } else if cfg!(windows) {
        let appdata = var("APPDATA").map(PathBuf::from).unwrap_or(home);
        (appdata.join("Gravewake"), vec![])
    } else {
        let data = var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        (
            data.join("gravewake"),
            vec![mac_support.join("Gravewake"), mac_support.join("Dark Veil")],
        )
    }
}

// Copy each legacy file once. Publishing with a hard link is atomic and cannot
// replace a newer Gravewake file, even if two game instances start together.
fn migrate_legacy_saves(destination: &Path, legacy: &[PathBuf]) -> std::io::Result<()> {
    for name in SAVE_FILES {
        let target = destination.join(name);
        if target.exists() {
            continue;
        }
        let mut found = None;
        for source in legacy {
            match std::fs::read(source.join(name)) {
                Ok(bytes) => {
                    found = Some(bytes);
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            }
        }
        let Some(bytes) = found else { continue };
        std::fs::create_dir_all(destination)?;
        let temporary = destination.join(format!("{name}.migration-{}.tmp", std::process::id()));
        std::fs::write(&temporary, bytes)?;
        let result = std::fs::hard_link(&temporary, &target);
        std::fs::remove_file(temporary)?;
        match result {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum Mode {
    Title,
    Arena,
    LevelUp,
    Shop,
    Pack,
    Tree,
    Collection,
    Bestiary,
    Paused,
    Dead,
    Victory,
}
pub use crate::weapons::WeaponKind;
use crate::weapons::{Effect, Model};
// Serde ignores the retired `foil` field in older saves. All loaded cards now
// use the standard finish and stats while retaining rarity and upgrades.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Card {
    pub kind: WeaponKind,
    pub rarity: usize,
    pub paths: [u8; 3],
    pub major: bool,
}
impl Card {
    pub fn starter() -> Self {
        Self {
            kind: WeaponKind::Pistol,
            rarity: 0,
            paths: [0; 3],
            major: false,
        }
    }
    pub fn name(&self) -> String {
        format!(
            "{} {}",
            ["WORN", "HELLFORGED", "CINDER", "REVENANT"][self.rarity.min(3)],
            self.kind.spec().name,
        )
    }
    pub fn damage(&self) -> f32 {
        let base = self.kind.spec().damage;
        base * [1., 1.25, 1.6, 2.1][self.rarity.min(3)] * (1. + self.paths[0] as f32 * 0.15)
    }
    pub fn capacity(&self) -> u32 {
        self.kind.spec().capacity
    }
    pub fn interval(&self) -> f32 {
        self.kind.spec().interval / (1. + self.paths[1] as f32 * 0.1)
    }
    pub fn reload_time(&self) -> f32 {
        self.kind.spec().reload / (1. + self.paths[1] as f32 * 0.1)
    }
    pub fn pellets(&self) -> u32 {
        self.kind.spec().pellets
    }
    /// Single-target damage per second over a full magazine and its reload,
    /// or per swing for melee. Elemental effects, piercing, splash and soul
    /// powers are not included; it is a like-for-like comparison of cards.
    pub fn sustained_dps(&self) -> f32 {
        if self.kind.melee() {
            return self.damage() / self.interval();
        }
        let rounds = self.capacity().max(1);
        let pulls = rounds.div_ceil(self.kind.spec().burst.max(1));
        let cycle = pulls as f32 * self.interval() + self.reload_time();
        rounds as f32 * self.damage() * self.pellets() as f32 / cycle
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Enemy {
    pub pos: Vec3,
    pub hp: f32,
    pub max_hp: f32,
    pub kind: usize,
    pub attack: f32,
    pub phase: f32,
    pub hit: f32,
    #[serde(default)]
    pub burn: f32,
    #[serde(default)]
    pub poison: f32,
    #[serde(default)]
    pub slow: f32,
    #[serde(default)]
    pub anatomy: Anatomy,
    #[serde(default)]
    pub ai: crate::encounters::Brain,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Run {
    #[serde(default)]
    pub survival: crate::survival::Survival,
    pub wave: u32,
    pub gold: u32,
    pub hp: f32,
    pub armor: f32,
    pub mana: f32,
    pub chalice: bool,
    pub weapon: Card,
    pub inventory: Vec<Card>,
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub enemies: Vec<Enemy>,
    pub kills: u32,
    pub ammo: u32,
    pub pack_buys: u32,
    pub draws: u32,
    pub offer: Card,
    pub choices: Vec<Card>,
    pub seed: u64,
    pub time: f32,
    #[serde(default)]
    pub stats: RunStats,
}
/// The kind of enemy attack that hurt the player.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Attack {
    Strike,
    Bolt,
    Slam,
    Burst,
}
/// What hurt the player: a species (index into the roster) and its attack.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Cause {
    pub kind: usize,
    pub attack: Attack,
}
impl Cause {
    /// For the death screen: "A GRAVE CRAWLER'S STRIKE", "THE TITHEKEEPER'S SLAM".
    pub fn describe(self) -> String {
        let name = crate::encounters::species(self.kind).name;
        let article = if name.starts_with("THE ") {
            ""
        } else if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
            "AN "
        } else {
            "A "
        };
        let attack = match self.attack {
            Attack::Strike => "STRIKE",
            Attack::Bolt => "BOLT",
            Attack::Slam => "SLAM",
            Attack::Burst => "BURST",
        };
        format!("{article}{name}'S {attack}")
    }
}
/// Statistics for the ending screen, saved with the run. Kills, the soul
/// level and the time already live in the run itself.
#[derive(Clone, Default, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunStats {
    pub headshots: u32,
    /// Health removed from enemies by your weapons, spells and powers.
    pub damage_dealt: f32,
    /// Vitality lost (armor absorbs the rest).
    pub damage_taken: f32,
    /// Vitality lost to each species, indexed like the roster.
    pub taken_from: Vec<f32>,
    /// The heaviest hit of the most recent wound: the killing blow at death.
    pub last_hit: Option<Cause>,
}
impl RunStats {
    /// Share `lost` vitality among this update's hits by their raw damage.
    pub fn record_wounds(&mut self, hits: &[(Vec3, Cause, f32)], lost: f32) {
        let total: f32 = hits.iter().map(|hit| hit.2).sum();
        if total <= 0. {
            return;
        }
        self.damage_taken += lost;
        for &(_, cause, amount) in hits {
            if self.taken_from.len() <= cause.kind {
                self.taken_from.resize(cause.kind + 1, 0.);
            }
            self.taken_from[cause.kind] += lost * amount / total;
        }
        self.last_hit = hits
            .iter()
            .max_by(|a, b| a.2.total_cmp(&b.2))
            .map(|hit| hit.1);
    }
    /// The species that took the most vitality, and its share of the total.
    pub fn top_source(&self) -> Option<(usize, f32)> {
        self.taken_from
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .filter(|(_, taken)| **taken > 0.)
            .map(|(kind, taken)| (kind, taken / self.damage_taken.max(*taken)))
    }
}
impl Default for Run {
    fn default() -> Self {
        Self {
            survival: crate::survival::Survival::default(),
            wave: 1,
            gold: 0,
            hp: 100.,
            armor: 0.,
            mana: 60.,
            chalice: false,
            weapon: Card::starter(),
            inventory: vec![Card::starter()],
            pos: Vec3::new(0., 1.65, 12.),
            yaw: 0.,
            pitch: 0.,
            enemies: vec![],
            kills: 0,
            ammo: 8,
            pack_buys: 0,
            draws: 0,
            offer: Card {
                kind: WeaponKind::Double,
                rarity: 2,
                ..Card::starter()
            },
            choices: vec![],
            seed: 948271,
            time: 0.,
            stats: RunStats::default(),
        }
    }
}
#[derive(Default)]
pub struct Input {
    pub forward: f32,
    pub right: f32,
    pub sprint: bool,
    pub fire: bool,
}
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub color: [f32; 3],
    pub size: f32,
}
pub struct Projectile {
    pub pos: Vec3,
    pub vel: Vec3,
    pub life: f32,
    pub damage: f32,
    pub kind: WeaponKind,
}
/// Reticle feedback for the player's own weapon hits, strongest first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum HitKind {
    Body,
    Head,
    Kill,
}
#[derive(Clone, Copy, Debug)]
pub struct HitMarker {
    pub kind: HitKind,
    pub life: f32,
}
pub const HIT_MARKER_LIFE: f32 = 0.3;
/// A fading arc around the reticle pointing toward a damage source.
#[derive(Clone, Copy, Debug)]
pub struct DamageMark {
    /// World bearing from the player, matching `forward()` at `yaw == bearing`.
    pub bearing: f32,
    pub life: f32,
}
pub const DAMAGE_MARK_LIFE: f32 = 1.2;
const MAX_DAMAGE_MARKS: usize = 4;
/// A special attack winding up toward the player out of view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Threat {
    /// World bearing from the player to the attacker, like `DamageMark`.
    pub bearing: f32,
    /// 0 as the warning starts, 1 as it lands; a dive in flight is 1.
    pub urgency: f32,
}
const MAX_THREATS: usize = 4;
pub struct Floater {
    pub pos: Vec3,
    pub text: String,
    pub life: f32,
}
// Player preferences that are not tied to graphics or presentation files.
// Missing fields take defaults so older or hand-edited files still load.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub sensitivity: f32,
    pub volume: f32,
    /// Music level, relative to `volume`.
    pub music_volume: f32,
    /// Vertical field of view in degrees.
    pub fov: f32,
    pub invert_y: bool,
    /// Softer hurt vignette and muzzle lighting for light-sensitive players.
    pub reduce_flashes: bool,
    #[serde(deserialize_with = "lenient")]
    pub bindings: Bindings,
    /// Show first-run field tips.
    pub field_tips: bool,
    /// One bit per `tips::Tip` already shown.
    pub tips_seen: u32,
}
/// Read a field, or use its default if that field alone is damaged, so one
/// bad entry doesn't reset every other preference.
fn lenient<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    Ok(serde_json::Value::deserialize(d)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default())
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            sensitivity: 0.0025,
            volume: 0.4,
            music_volume: 0.5,
            fov: 70.,
            invert_y: false,
            reduce_flashes: false,
            bindings: Bindings::default(),
            field_tips: true,
            tips_seen: 0,
        }
    }
}
impl Preferences {
    pub const FOV_RANGE: (f32, f32) = (60., 90.);
    pub const SENSITIVITY_RANGE: (f32, f32) = (0.0007, 0.007);
    fn sanitized(self) -> Self {
        let d = Self::default();
        let clean = |x: f32, (lo, hi): (f32, f32), fallback: f32| {
            if x.is_finite() {
                x.clamp(lo, hi)
            } else {
                fallback
            }
        };
        Self {
            sensitivity: clean(self.sensitivity, Self::SENSITIVITY_RANGE, d.sensitivity),
            volume: clean(self.volume, (0., 1.), d.volume),
            music_volume: clean(self.music_volume, (0., 1.), d.music_volume),
            fov: clean(self.fov, Self::FOV_RANGE, d.fov),
            invert_y: self.invert_y,
            reduce_flashes: self.reduce_flashes,
            bindings: self.bindings,
            field_tips: self.field_tips,
            tips_seen: self.tips_seen,
        }
    }
    pub(crate) fn from_json(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice::<Self>(bytes)
            .ok()
            .map(Self::sanitized)
    }
}
/// Lifetime bests, stored in records.json beside the run save.
#[derive(Clone, Copy, Default, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Records {
    pub runs: u32,
    /// Deepest descent entered, including endless descents.
    pub deepest: u32,
    pub most_souls: u32,
    pub victories: u32,
    /// Fastest twelve-descent victory, in seconds of run time.
    pub fastest_victory: Option<f32>,
}
impl Records {
    /// Fold a run's progress in and return the names of records it beat.
    pub fn record(
        &mut self,
        wave: u32,
        souls: u32,
        victory_time: Option<f32>,
    ) -> Vec<&'static str> {
        let mut beaten = vec![];
        if wave > self.deepest {
            self.deepest = wave;
            beaten.push("DEEPEST DESCENT");
        }
        if souls > self.most_souls {
            self.most_souls = souls;
            beaten.push("MOST SOULS");
        }
        if let Some(time) = victory_time {
            self.victories += 1;
            if self.fastest_victory.is_none_or(|best| time < best) {
                self.fastest_victory = Some(time);
                beaten.push("FASTEST VICTORY");
            }
        }
        beaten
    }
}
#[derive(Serialize, Deserialize)]
struct Save {
    version: u32,
    mode: Mode,
    run: Run,
}
pub struct Game {
    pub mode: Mode,
    pub return_mode: Mode,
    pub run: Run,
    pub input: Input,
    pub particles: Vec<Particle>,
    pub floaters: Vec<Floater>,
    pub projectiles: Vec<Projectile>,
    pub hazards: Vec<crate::encounters::Hazard>,
    pub burst_remaining: u32,
    pub burst_timer: f32,
    pub spin: f32,
    pub attack_serial: u32,
    pub melee_pending: bool,
    pub melee_timer: f32,
    pub cooldown: f32,
    pub reload: f32,
    pub flash: f32,
    pub shot_age: f32,
    pub look_sway: Vec2,
    pub motion_speed: f32,
    /// Sounds emitted in the world, played with panning and distance.
    pub world_sounds: Vec<(&'static str, Vec3)>,
    pub hurt: f32,
    pub hit_marker: Option<HitMarker>,
    pub damage_marks: Vec<DamageMark>,
    pub dash: f32,
    pub dash_cd: f32,
    pub spell_cd: f32,
    pub elapsed: f32,
    pub notice: String,
    pub notice_time: f32,
    /// The field tip on screen, and tips waiting their turn.
    pub tip: Option<crate::tips::ActiveTip>,
    pub tip_queue: Vec<crate::tips::Tip>,
    /// Field tips run only in real play, never in smoke or review runs.
    pub tips_enabled: bool,
    /// The input prompts describe: the keyboard or the controller used last.
    pub device: Device,
    pub sound_events: Vec<&'static str>,
    pub spawned_bodies: Vec<BodyEvent>,
    pub physics_impacts: Vec<anatomy::PhysicsImpact>,
    pub records: Records,
    /// Records this run has set so far, shown on the ending screen.
    pub run_records: Vec<&'static str>,
    pub prefs: Preferences,
    pub shader_intensity: f32,
    pub vsync: bool,
    pub show_fps: bool,
    pub fps: f32,
    pub settings: bool,
    /// The journal's Controls page is open instead of Preferences.
    pub journal_controls: bool,
    /// An action waiting for its new key on the Controls page.
    pub rebinding: Option<Action>,
    /// The Controls page's latest confirmation or refusal.
    pub controls_note: String,
    pub quit_requested: bool,
    pub confirm_new_run: bool,
    pub has_save: bool,
    pub bestiary_index: usize,
    pub collection_kind: usize,
    pub collection_group: usize,
    pub collection_rarity: usize,
    pub practice_backup: Option<Run>,
    pub save_enabled: bool,
    pub(crate) navigation: world_layout::Navigation,
}
impl Game {
    pub fn new(save_enabled: bool) -> Self {
        if save_enabled {
            let (destination, legacy) = Self::dirs();
            if let Err(e) = migrate_legacy_saves(&destination, &legacy) {
                eprintln!("Could not migrate legacy saves; original files are preserved: {e}");
            }
        }
        Self {
            mode: Mode::Title,
            return_mode: Mode::Title,
            run: Run::default(),
            input: Input::default(),
            particles: vec![],
            floaters: vec![],
            projectiles: vec![],
            hazards: vec![],
            burst_remaining: 0,
            burst_timer: 0.,
            spin: 0.,
            attack_serial: 0,
            melee_pending: false,
            melee_timer: 0.,
            cooldown: 0.,
            reload: 0.,
            flash: 0.,
            shot_age: 10.,
            look_sway: Vec2::ZERO,
            motion_speed: 0.,
            world_sounds: vec![],
            hurt: 0.,
            hit_marker: None,
            damage_marks: vec![],
            dash: 0.,
            dash_cd: 0.,
            spell_cd: 0.,
            elapsed: 0.,
            notice: String::new(),
            notice_time: 0.,
            tip: None,
            tip_queue: vec![],
            tips_enabled: save_enabled,
            device: Device::Keyboard,
            sound_events: vec![],
            spawned_bodies: vec![],
            physics_impacts: vec![],
            records: if save_enabled {
                std::fs::read(Self::load_path("records.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice(&b).ok())
                    .unwrap_or_default()
            } else {
                Records::default()
            },
            run_records: vec![],
            prefs: if save_enabled {
                std::fs::read(Self::load_path("settings.json"))
                    .ok()
                    .and_then(|b| Preferences::from_json(&b))
                    .unwrap_or_default()
            } else {
                Preferences::default()
            },
            shader_intensity: if save_enabled {
                std::fs::read(Self::load_path("graphics.json"))
                    .ok()
                    .and_then(|b| serde_json::from_slice::<f32>(&b).ok())
                    .filter(|x| x.is_finite())
                    .unwrap_or(1.)
                    .clamp(0., 1.)
            } else {
                1.
            },
            vsync: if save_enabled {
                Self::performance_settings().0
            } else {
                false
            },
            show_fps: if save_enabled {
                Self::performance_settings().1
            } else {
                false
            },
            fps: 0.,
            settings: false,
            journal_controls: false,
            rebinding: None,
            controls_note: String::new(),
            quit_requested: false,
            confirm_new_run: false,
            has_save: Self::load_path("run.json").exists(),
            bestiary_index: 0,
            collection_kind: 1,
            collection_group: 1,
            collection_rarity: 2,
            practice_backup: None,
            save_enabled,
            navigation: world_layout::Navigation::default(),
        }
    }
    fn dirs() -> (PathBuf, Vec<PathBuf>) {
        save_dirs(|name| std::env::var_os(name))
    }
    pub fn save_path() -> PathBuf {
        Self::dirs().0.join("run.json")
    }
    // Fall back to an older location if migration could not copy the file.
    fn load_path(name: &str) -> PathBuf {
        let (current, legacy) = Self::dirs();
        std::iter::once(current)
            .chain(legacy)
            .map(|dir| dir.join(name))
            .find(|path| path.exists())
            .unwrap_or_else(|| Self::save_path().with_file_name(name))
    }
    fn performance_settings() -> (bool, bool) {
        std::fs::read(Self::load_path("performance.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or((false, false))
    }
    pub fn save_performance(&self) {
        if !self.save_enabled {
            return;
        }
        let path = Self::save_path().with_file_name("performance.json");
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let temp = path.with_extension("tmp");
            std::fs::write(&temp, serde_json::to_vec(&(self.vsync, self.show_fps))?)?;
            std::fs::rename(temp, path)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("Could not save performance settings: {e}");
        }
    }
    // Persist journal settings: player preferences and the Hollowlight level.
    pub fn save_preferences(&self) {
        if !self.save_enabled {
            return;
        }
        let path = Self::save_path().with_file_name("settings.json");
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let data = serde_json::to_vec_pretty(&self.prefs.sanitized())?;
            let temporary = path.with_extension("tmp");
            std::fs::write(&temporary, data)?;
            std::fs::rename(temporary, path)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("Could not save preferences: {e}");
        }
        self.save_graphics();
    }
    fn save_graphics(&self) {
        if !self.save_enabled {
            return;
        }
        let path = Self::save_path().with_file_name("graphics.json");
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let data = serde_json::to_vec(&self.shader_intensity.clamp(0., 1.))?;
            let temporary = path.with_extension("tmp");
            std::fs::write(&temporary, data)?;
            std::fs::rename(temporary, path)?;
            Ok(())
        })();
        if let Err(e) = result {
            eprintln!("Could not save graphics preference: {e}");
        }
    }
    pub fn save(&mut self) -> bool {
        self.save_to(&Self::save_path())
    }
    fn save_to(&mut self, path: &Path) -> bool {
        if !self.save_enabled || self.mode == Mode::Title || self.practice_backup.is_some() {
            return true;
        }
        let mode = match self.mode {
            Mode::Paused => Mode::Arena,
            Mode::Collection | Mode::Bestiary => self.return_mode,
            Mode::Tree => Mode::Shop,
            x => x,
        };
        if mode == Mode::Title {
            return true;
        }
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let data = serde_json::to_vec_pretty(&Save {
                version: 1,
                mode,
                run: self.run.clone(),
            })?;
            let tmp = path.with_extension("tmp");
            std::fs::write(&tmp, data)?;
            std::fs::rename(tmp, path)?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.has_save = true;
                true
            }
            Err(e) => {
                self.notify(&format!("Could not save: {e}"));
                false
            }
        }
    }
    pub fn resume_save(&mut self) {
        self.run_records.clear();
        match std::fs::read(Self::load_path("run.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<Save>(&b).ok())
        {
            Some(s)
                if s.version == 1
                    && s.run.weapon.rarity < 4
                    && s.run.wave >= 1
                    && s.run.wave <= 10000 =>
            {
                self.run = s.run;
                self.run.pos = world_layout::resolve_position(self.run.pos, PLAYER_RADIUS);
                for enemy in &mut self.run.enemies {
                    enemy.pos = world_layout::resolve_position(enemy.pos, ENEMY_RADIUS);
                }
                self.mode = s.mode;
                self.reset_effects();
            }
            _ => self.notify("This save could not be loaded."),
        }
    }
    fn reset_effects(&mut self) {
        self.input = Input::default();
        self.particles.clear();
        self.spawned_bodies.clear();
        self.physics_impacts.clear();
        self.floaters.clear();
        self.projectiles.clear();
        self.hazards.clear();
        self.burst_remaining = 0;
        self.melee_pending = false;
        self.spin = 0.;
        self.cooldown = 0.;
        self.reload = 0.;
        self.flash = 0.;
        self.shot_age = 10.;
        self.look_sway = Vec2::ZERO;
        self.motion_speed = 0.;
        self.hurt = 0.;
        self.hit_marker = None;
        self.damage_marks.clear();
        self.dash = 0.;
        self.dash_cd = 0.;
        self.spell_cd = 0.;
    }
    pub fn new_run(&mut self) {
        // Player runs get their own seed; tests, smoke and reviews disable
        // saving and keep the fixed default seed so they stay reproducible.
        let seed = if self.save_enabled {
            fresh_seed()
        } else {
            Run::default().seed
        };
        self.new_run_with_seed(seed);
        self.records.runs += 1;
        self.run_records.clear();
        self.track_records(None);
    }
    // Fold the current run into lifetime records; practice never counts.
    fn track_records(&mut self, victory_time: Option<f32>) {
        if self.practice_backup.is_some() {
            return;
        }
        for name in self
            .records
            .record(self.run.wave, self.run.kills, victory_time)
        {
            if !self.run_records.contains(&name) {
                self.run_records.push(name);
            }
        }
        self.save_records();
    }
    fn save_records(&self) {
        if self.save_enabled {
            if let Err(e) = self.write_records(&Self::save_path().with_file_name("records.json")) {
                eprintln!("Could not save records: {e}");
            }
        }
    }
    fn write_records(&self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let data = serde_json::to_vec_pretty(&self.records)?;
        let temporary = path.with_extension("tmp");
        std::fs::write(&temporary, data)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }
    fn new_run_with_seed(&mut self, seed: u64) {
        self.run = Run {
            seed,
            ..Run::default()
        };
        self.reset_effects();
        self.start_wave();
    }
    pub fn rand(&mut self) -> f32 {
        self.run.seed = self
            .run
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.run.seed >> 33) as f32) / (u32::MAX as f32 / 2.)
    }
    pub fn roll_card(&mut self) -> Card {
        let r = self.rand();
        let rarity = if r < 0.58 {
            0
        } else if r < 0.86 {
            1
        } else if r < 0.98 {
            2
        } else {
            3
        };
        let k = (self.rand() * WeaponKind::ALL.len() as f32) as usize;
        Card {
            kind: WeaponKind::ALL[k.min(WeaponKind::ALL.len() - 1)],
            rarity,
            paths: [0; 3],
            major: false,
        }
    }
    pub fn start_wave(&mut self) {
        self.mode = Mode::Arena;
        self.run.pos = Vec3::new(0., 1.65, 12.);
        self.run.yaw = 0.;
        self.run.pitch = 0.;
        self.run.ammo = self.run.weapon.capacity();
        self.run.mana = 60.;
        self.run.enemies.clear();
        self.reset_effects();
        self.seed_encounter();
        self.notify(&format!("DESCENT {:02}  /  THE BELL TOLLS", self.run.wave));
        self.save();
    }
    pub fn forward(&self) -> Vec3 {
        Vec3::new(
            self.run.yaw.sin() * self.run.pitch.cos(),
            self.run.pitch.sin(),
            -self.run.yaw.cos() * self.run.pitch.cos(),
        )
    }
    pub fn reload_weapon(&mut self) {
        if self.mode == Mode::Arena
            && self.reload <= 0.
            && !self.run.weapon.kind.melee()
            && self.burst_remaining == 0
            && self.run.ammo < self.run.weapon.capacity()
        {
            self.reload = self.run.weapon.reload_time();
            self.sound_events.push("cloth");
        }
    }
    pub fn dodge(&mut self) {
        if self.mode == Mode::Arena && self.dash_cd <= 0. {
            self.dash = 0.2;
            self.dash_cd = 1.5;
            self.sound_events.push("dash");
        }
    }
    /// Give the action waiting on the Controls page its new input.
    pub fn bind(&mut self, trigger: Trigger, glyph: Option<char>) {
        let Some(action) = self.rebinding else {
            return;
        };
        let bindings = &mut self.prefs.bindings;
        self.controls_note = match bindings.assign(action, trigger, glyph) {
            Ok(displaced) => {
                self.rebinding = None;
                let mut note = format!("{} is now {}.", action.name(), bindings.label(action));
                if let Some(other) = displaced {
                    note += &format!(" {} moved to {}.", other.name(), bindings.label(other));
                }
                note
            }
            // Keep waiting so the player can simply press another key.
            Err(reason) => format!("{reason} Choose another key for {}.", action.name()),
        };
        self.save_preferences();
    }
    /// The input that performs `action` on the device the player used last.
    pub fn prompt(&self, action: Action) -> String {
        match self.device {
            Device::Keyboard => self.prefs.bindings.label(action),
            Device::Controller => action.pad_label().into(),
        }
    }
    pub fn movement_prompt(&self) -> String {
        match self.device {
            Device::Keyboard => self.prefs.bindings.movement_label(),
            Device::Controller => Action::Forward.pad_label().into(),
        }
    }
    /// Escape, or Start on a controller, pauses and goes back.
    pub fn back_prompt(&self) -> &'static str {
        match self.device {
            Device::Keyboard => "Escape",
            Device::Controller => "Start",
        }
    }
    /// Firing isn't rebindable: the left mouse button or the right trigger.
    pub fn fire_prompt(&self) -> &'static str {
        match self.device {
            Device::Keyboard => "LMB",
            Device::Controller => "RT",
        }
    }
    pub fn notify(&mut self, s: &str) {
        self.notice = s.into();
        self.notice_time = 3.4;
    }
    pub fn fire(&mut self, spell: bool) {
        if self.mode != Mode::Arena {
            return;
        }
        if spell {
            if !self.run.chalice {
                self.notify("Bind a Hollow Chalice to cast Ember Bolt.");
                return;
            }
            if self.spell_cd > 0. || self.run.mana < 12. {
                return;
            }
            self.run.mana -= 12.;
            self.spell_cd = 0.7;
            self.sound_events.push("spell");
            self.projectiles.push(Projectile {
                pos: self.run.pos + self.forward() * 0.5,
                vel: self.forward() * 22.,
                life: 2.,
                damage: 95.,
                kind: WeaponKind::EmberStaff,
            });
            return;
        }
        if self.cooldown > 0. || self.reload > 0. || self.burst_remaining > 0 {
            return;
        }
        let kind = self.run.weapon.kind;
        if kind.melee() {
            self.weapon_melee();
            return;
        }
        if self.run.ammo == 0 {
            self.reload_weapon();
            return;
        }
        self.cooldown = self.run.weapon.interval() / self.haste()
            * if kind == WeaponKind::Gatling {
                1. - self.spin * 0.65
            } else {
                1.
            };
        self.burst_remaining = (kind.spec().burst - 1).min(self.run.ammo.saturating_sub(1));
        self.burst_timer = 0.075;
        self.emit_shot();
    }
    fn emit_shot(&mut self) {
        if self.run.ammo == 0 {
            self.burst_remaining = 0;
            return;
        }
        self.run.ammo -= 1;
        self.shot_age = 0.;
        self.flash = 0.095;
        self.attack_serial += 1;
        let kind = self.run.weapon.kind;
        let s = kind.spec();
        self.sound_events.push(if s.group == 1 {
            "shotgun"
        } else if s.group == 4 {
            "spell"
        } else if matches!(s.model, Model::Bow | Model::Crossbow) {
            "bow"
        } else {
            "shot"
        });
        let forward = self.forward();
        for _ in 0..s.pellets {
            let dir = (forward
                + Vec3::new(
                    (self.rand() - 0.5) * s.spread,
                    (self.rand() - 0.5) * s.spread,
                    0.,
                ))
            .normalize();
            if s.projectile > 0. {
                let lob = if s.group == 3 || kind == WeaponKind::PlagueCenser {
                    Vec3::Y * 4.
                } else {
                    Vec3::ZERO
                };
                self.projectiles.push(Projectile {
                    pos: self.run.pos + dir * 0.55,
                    vel: dir * s.projectile + lob,
                    life: s.range / s.projectile + 0.7,
                    damage: self.run.weapon.damage() * self.damage_multiplier(),
                    kind,
                });
            } else {
                self.ray_attack(
                    self.run.pos,
                    dir,
                    kind,
                    self.run.weapon.damage() * self.damage_multiplier(),
                );
            }
        }
        self.collect_dead();
    }
    fn ray_attack(&mut self, origin: Vec3, dir: Vec3, kind: WeaponKind, damage: f32) {
        let s = kind.spec();
        let range = world_layout::obstruction(origin, origin + dir * s.range).unwrap_or(s.range);
        let mut hits: Vec<_> = self
            .run
            .enemies
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if e.hp <= 0. {
                    return None;
                }
                anatomy::trace(e, self.run.pos, origin, dir, range, self.run.time)
                    .map(|(part, t)| (i, t, part))
            })
            .collect();
        hits.sort_by(|a, b| a.1.total_cmp(&b.1));
        self.physics_impacts.push(anatomy::PhysicsImpact::Ray {
            origin,
            direction: dir,
            range: hits.first().map_or(range, |hit| hit.1),
            energy: damage,
        });
        let limit = if s.effect == Effect::Pierce {
            match kind {
                WeaponKind::Duelist => 2,
                WeaponKind::SlugGun => 3,
                _ => 4,
            }
        } else {
            1
        };
        for (n, &(i, distance, part)) in hits.iter().take(limit).enumerate() {
            self.apply_hit_at(
                i,
                damage * 0.82f32.powi(n as i32),
                kind,
                dir,
                part,
                origin + dir * distance,
            );
            if s.effect == Effect::Chain {
                self.chain_from(i, kind, damage);
            }
            if s.group == 4 || s.effect == Effect::Pierce {
                self.trail(origin, origin + dir * distance, kind.tint());
            }
        }
        if hits.is_empty() && s.group == 4 {
            self.trail(origin, origin + dir * range, kind.tint());
        }
    }
    fn apply_hit(&mut self, i: usize, damage: f32, kind: WeaponKind, dir: Vec3) {
        let part = if crate::encounters::head_only(self.run.enemies[i].kind) {
            Part::Head
        } else {
            Part::Torso
        };
        let point = Pose::for_enemy(&self.run.enemies[i], self.run.pos).anchor(part);
        self.apply_hit_at(i, damage, kind, dir, part, point);
    }
    fn apply_hit_at(
        &mut self,
        i: usize,
        damage: f32,
        kind: WeaponKind,
        dir: Vec3,
        part: Part,
        point: Vec3,
    ) {
        if self.run.enemies[i].hp <= 0. || self.run.enemies[i].anatomy.missing(part) {
            return;
        }
        let armor = if self.run.enemies[i].kind == 6 && part == Part::Torso {
            0.45
        } else {
            1.
        };
        let damage = damage * armor;
        let multiplier = match part {
            Part::Head => 2.,
            Part::Torso => 1.,
            _ => 0.8,
        };
        let actual = (damage * multiplier).min(self.run.enemies[i].hp.max(0.));
        let s = kind.spec();
        let fracture_allowed = matches!(s.group, 1 | 3)
            || matches!(
                kind,
                WeaponKind::HandCannon
                    | WeaponKind::SlugGun
                    | WeaponKind::Warhammer
                    | WeaponKind::ChainFlail
            );
        self.damage_enemy_at(
            i,
            damage * multiplier,
            damage,
            dir,
            part,
            point,
            fracture_allowed,
        );
        self.register_hit(i, part);
        let e = &mut self.run.enemies[i];
        let flat = Vec3::new(dir.x, 0., dir.z).normalize_or_zero();
        if e.hp > 0. {
            e.pos = world_layout::move_body(e.pos, flat * (s.knockback - 0.1), ENEMY_RADIUS);
        }
        e.anatomy.impulse = dir * (3. + s.knockback * 3. + damage * 0.018).min(12.);
        match s.effect {
            Effect::Burn => e.burn = 3.,
            Effect::Poison => e.poison = (e.poison + 1.8).min(6.),
            Effect::Frost => e.slow = 3.,
            Effect::Drain => self.run.hp = (self.run.hp + actual * 0.12).min(self.max_hp()),
            _ => {}
        }
    }
    fn chain_from(&mut self, first: usize, kind: WeaponKind, damage: f32) {
        let mut seen = vec![first];
        let mut point = self.run.enemies[first].pos + Vec3::Y * 1.2;
        for n in 0..if kind == WeaponKind::StormWand { 3 } else { 2 } {
            let next = self
                .run
                .enemies
                .iter()
                .enumerate()
                .filter(|(i, e)| {
                    !seen.contains(i)
                        && e.hp > 0.
                        && world_layout::obstruction(point, e.pos + Vec3::Y * 1.2).is_none()
                })
                .map(|(i, e)| (i, (e.pos + Vec3::Y * 1.2 - point).length()))
                .filter(|(_, d)| *d < kind.spec().radius)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, _)) = next {
                let target = self.run.enemies[i].pos + Vec3::Y * 1.2;
                self.trail(point, target, kind.tint());
                self.apply_hit(
                    i,
                    damage * 0.72f32.powi(n + 1),
                    kind,
                    (target - point).normalize_or_zero(),
                );
                seen.push(i);
                point = target;
            } else {
                break;
            }
        }
    }
    pub(crate) fn trail(&mut self, a: Vec3, b: Vec3, color: [f32; 3]) {
        for j in 0..18 {
            self.particles.push(Particle {
                pos: a.lerp(b, j as f32 / 17.),
                vel: Vec3::ZERO,
                life: 0.14,
                color,
                size: 0.024,
            });
        }
    }
    fn blast(
        &mut self,
        p: Vec3,
        kind: WeaponKind,
        damage: f32,
        direct: Option<(usize, Part, Vec3)>,
    ) {
        let radius = kind.spec().radius;
        self.physics_impacts.push(anatomy::PhysicsImpact::Blast {
            center: p,
            radius,
            energy: damage,
        });
        for i in 0..self.run.enemies.len() {
            if self.run.enemies[i].hp <= 0. {
                continue;
            }
            if let Some((index, part, dir)) = direct {
                if i == index {
                    self.apply_hit_at(i, damage, kind, dir, part, p);
                    continue;
                }
            }
            // Splash starts at the closest surviving body surface: a ground blast
            // can break a leg, while a direct skull impact keeps its head hit.
            let e = &self.run.enemies[i];
            let pose = Pose::for_enemy(e, self.run.pos);
            let nearest = Part::ALL
                .into_iter()
                .filter(|part| anatomy::present(e, *part))
                .flat_map(|part| {
                    pose.segments(part).into_iter().map(move |(a, b, r)| {
                        let ab = b - a;
                        let point = a + ab
                            * ((p - a).dot(ab) / ab.length_squared().max(0.0001)).clamp(0., 1.);
                        (part, point, ((point - p).length() - r).max(0.))
                    })
                })
                .min_by(|a, b| a.2.total_cmp(&b.2));
            if let Some((part, point, d)) = nearest {
                if d < radius && world_layout::obstruction(p, point).is_none() {
                    self.apply_hit_at(
                        i,
                        damage * (1. - 0.65 * d / radius),
                        kind,
                        (point - p).normalize_or_zero(),
                        part,
                        point,
                    );
                }
            }
        }
        for j in 0..40 {
            let a = j as f32 * 2.39996;
            let v = Vec3::new(a.cos(), (j as f32 * 1.72).sin() * 0.5, a.sin());
            self.particles.push(Particle {
                pos: p,
                vel: v * radius * 2.,
                life: 0.4,
                color: kind.tint(),
                size: 0.06,
            });
        }
        self.sound_events.push(if kind.spec().group == 4 {
            "spell"
        } else {
            "impact"
        });
    }
    fn update_projectiles(&mut self, dt: f32) {
        let mut live = std::mem::take(&mut self.projectiles);
        for p in &mut live {
            let start = p.pos;
            let step = p.vel * dt;
            p.pos += step;
            p.life -= dt;
            if p.kind.spec().group == 3 || p.kind == WeaponKind::PlagueCenser {
                p.vel.y -= 9.8 * dt;
            }
            let dir = step.normalize_or_zero();
            let wall_hit = world_layout::obstruction(start, p.pos);
            let range = wall_hit.unwrap_or(step.length());
            let hit = self
                .run
                .enemies
                .iter()
                .enumerate()
                .filter_map(|(i, e)| {
                    if e.hp <= 0. {
                        return None;
                    }
                    anatomy::trace(e, self.run.pos, start, dir, range, self.run.time)
                        .map(|(part, t)| (i, t, part))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if hit.is_some() || wall_hit.is_some() || p.pos.y < 0.15 || p.life <= 0. {
                if let Some(along) = wall_hit {
                    p.pos = start + dir * (along - 0.02).max(0.);
                }
                if let Some((_, along, _)) = hit {
                    p.pos = start + dir * along;
                }
                if p.kind.spec().radius > 0. {
                    self.blast(
                        p.pos,
                        p.kind,
                        p.damage,
                        hit.map(|(i, _, part)| (i, part, dir)),
                    );
                } else if let Some((i, _, part)) = hit {
                    if p.kind.spec().effect == Effect::Pierce {
                        self.ray_attack(start - dir, dir, p.kind, p.damage);
                    } else {
                        self.apply_hit_at(i, p.damage, p.kind, dir, part, p.pos);
                    }
                }
                p.life = 0.;
            }
        }
        live.retain(|p| p.life > 0.);
        self.projectiles = live;
        self.collect_dead();
    }
    fn weapon_melee(&mut self) {
        let kind = self.run.weapon.kind;
        let s = kind.spec();
        self.cooldown = self.run.weapon.interval() / self.haste();
        self.shot_age = 0.;
        self.attack_serial += 1;
        self.sound_events.push(if s.model == Model::Hammer {
            "heavy_swing"
        } else {
            "swing"
        });
        self.melee_pending = true;
        self.melee_timer = self.run.weapon.interval() * 0.28;
    }
    fn resolve_melee(&mut self) {
        let kind = self.run.weapon.kind;
        let s = kind.spec();
        self.physics_impacts.push(anatomy::PhysicsImpact::Ray {
            origin: self.run.pos,
            direction: self.forward(),
            range: s.range,
            energy: self.run.weapon.damage() * self.damage_multiplier(),
        });
        let forward = Vec3::new(self.forward().x, 0., self.forward().z).normalize_or_zero();
        if kind == WeaponKind::Rapier {
            self.run.pos = world_layout::move_body(self.run.pos, forward * 0.42, PLAYER_RADIUS);
        }
        for i in 0..self.run.enemies.len() {
            let diff = self.run.enemies[i].pos - self.run.pos;
            let flat = Vec3::new(diff.x, 0., diff.z);
            if flat.length() < s.range && flat.normalize_or_zero().dot(forward) > s.spread {
                let e = &self.run.enemies[i];
                let pose = Pose::for_enemy(e, self.run.pos);
                let aim = self.forward();
                let (part, point) = Part::ALL
                    .into_iter()
                    .filter(|p| anatomy::present(e, *p))
                    .map(|p| {
                        let segments = pose.segments(p);
                        let (a, b, _) = segments[0];
                        (p, (a + b) * 0.5)
                    })
                    .min_by(|a, b| {
                        let miss = |p: Vec3| {
                            let v = p - self.run.pos;
                            (v - aim * v.dot(aim).max(0.)).length_squared()
                        };
                        miss(a.1).total_cmp(&miss(b.1))
                    })
                    .unwrap();
                if point.distance(self.run.pos) > s.range + 0.25
                    || world_layout::obstruction(self.run.pos, point).is_some()
                {
                    continue;
                }
                self.apply_hit_at(
                    i,
                    self.run.weapon.damage() * self.damage_multiplier(),
                    kind,
                    forward,
                    part,
                    point,
                );
                self.sound_events.push("blade_hit");
            }
        }
        if kind == WeaponKind::Warhammer {
            self.blast(
                self.run.pos + forward * 2. - Vec3::Y * 1.2,
                kind,
                self.run.weapon.damage() * self.damage_multiplier() * 0.35,
                None,
            );
        }
        self.collect_dead();
    }
    pub fn melee(&mut self) {
        if self.mode != Mode::Arena || self.cooldown > 0. {
            return;
        }
        if self.run.weapon.kind.melee() {
            self.weapon_melee();
            return;
        }
        self.cooldown = 0.6;
        self.flash = 0.08;
        self.sound_events.push("shot");
        let forward = self.forward();
        for i in 0..self.run.enemies.len() {
            let diff = self.run.enemies[i].pos - self.run.pos;
            if diff.length() < 3.
                && diff.normalize().dot(forward) > 0.1
                && world_layout::obstruction(self.run.pos, self.run.enemies[i].pos + Vec3::Y)
                    .is_none()
                && self.run.enemies[i].hp > 0.
            {
                self.damage_enemy(i, 45., forward);
                self.register_hit(i, Part::Torso);
            }
        }
        self.collect_dead();
    }
    // Call after a weapon hit lands on a living enemy. Pellets and splash
    // landing together keep the strongest result and a single kill tick.
    fn register_hit(&mut self, i: usize, part: Part) {
        let e = &self.run.enemies[i];
        let head = part == Part::Head && !crate::encounters::head_only(e.kind);
        if head {
            self.run.stats.headshots += 1;
        }
        let kind = if e.hp <= 0. {
            HitKind::Kill
        } else if head {
            HitKind::Head
        } else {
            HitKind::Body
        };
        let fresh = self
            .hit_marker
            .filter(|m| m.life > HIT_MARKER_LIFE - 0.05)
            .map(|m| m.kind);
        if kind == HitKind::Kill && fresh != Some(HitKind::Kill) {
            self.sound_events.push("kill");
        }
        self.hit_marker = Some(HitMarker {
            kind: fresh.map_or(kind, |k| k.max(kind)),
            life: HIT_MARKER_LIFE,
        });
    }
    /// Point a damage arc toward `source`. Nearby bearings refresh one arc.
    pub(crate) fn mark_damage_from(&mut self, source: Vec3) {
        let flat = (source - self.run.pos) * Vec3::new(1., 0., 1.);
        if flat.length_squared() < 0.01 {
            return;
        }
        let bearing = flat.x.atan2(-flat.z);
        let near = |m: &DamageMark| {
            let d = bearing - m.bearing;
            d.sin().atan2(d.cos()).abs() < 0.35
        };
        if let Some(mark) = self.damage_marks.iter_mut().find(|m| near(m)) {
            *mark = DamageMark {
                bearing,
                life: DAMAGE_MARK_LIFE,
            };
            return;
        }
        if self.damage_marks.len() >= MAX_DAMAGE_MARKS {
            let oldest = (0..self.damage_marks.len())
                .min_by(|&a, &b| {
                    self.damage_marks[a]
                        .life
                        .total_cmp(&self.damage_marks[b].life)
                })
                .unwrap();
            self.damage_marks.remove(oldest);
        }
        self.damage_marks.push(DamageMark {
            bearing,
            life: DAMAGE_MARK_LIFE,
        });
    }
    /// Wind-ups aimed at the player from more than `half_view` radians off
    /// the view's centre, most urgent first. In-view attacks have their own
    /// visible warnings.
    pub fn unseen_threats(&self, half_view: f32) -> Vec<Threat> {
        let mut threats: Vec<Threat> = self
            .run
            .enemies
            .iter()
            .filter(|e| e.hp > 0.)
            .filter_map(|e| {
                let urgency = if e.ai.charging > 0. && matches!(e.kind, 4 | 9) {
                    1.
                } else if e.ai.warning > 0.
                    && crate::encounters::aimed_at(e.kind, e.ai.target, self.run.pos)
                {
                    (1. - e.ai.warning / crate::encounters::warning_time(e.kind)).clamp(0., 1.)
                } else {
                    return None;
                };
                let flat = (e.pos - self.run.pos) * Vec3::new(1., 0., 1.);
                if flat.length_squared() < 0.01 {
                    return None;
                }
                let bearing = flat.x.atan2(-flat.z);
                let relative = bearing - self.run.yaw;
                (relative.sin().atan2(relative.cos()).abs() > half_view)
                    .then_some(Threat { bearing, urgency })
            })
            .collect();
        threats.sort_by(|a, b| b.urgency.total_cmp(&a.urgency));
        threats.truncate(MAX_THREATS);
        threats
    }
    pub(crate) fn damage_enemy(&mut self, i: usize, damage: f32, dir: Vec3) {
        let part = if crate::encounters::head_only(self.run.enemies[i].kind) {
            Part::Head
        } else {
            Part::Torso
        };
        let point = Pose::for_enemy(&self.run.enemies[i], self.run.pos).anchor(part);
        self.damage_enemy_at(i, damage, damage, dir, part, point, false);
    }
    fn damage_enemy_at(
        &mut self,
        i: usize,
        damage: f32,
        structural: f32,
        dir: Vec3,
        part: Part,
        point: Vec3,
        fracture_allowed: bool,
    ) {
        if self.run.enemies[i].hp <= 0. || self.run.enemies[i].anatomy.missing(part) {
            return;
        }
        // All pellets in one update hit the same visible pose. Intermediate
        // knockback/flinch must not move the handoff before a frame is rendered.
        let same_frame = self.run.enemies[i].anatomy.impact_time == self.run.time;
        let before = if same_frame {
            self.run.enemies[i].anatomy.impact_pose.as_deref().cloned()
        } else {
            None
        }
        .unwrap_or_else(|| Pose::for_enemy(&self.run.enemies[i], self.run.pos));
        let e = &mut self.run.enemies[i];
        if !same_frame || e.anatomy.impact_pose.is_none() {
            e.anatomy.impact_pose = Some(Box::new(before.clone()));
        }
        let same_impact = self.run.time - e.anatomy.impact_time < 0.06;
        e.anatomy.impact_energy = if same_impact {
            e.anatomy.impact_energy + structural
        } else {
            structural
        };
        e.anatomy.impact_time = self.run.time;
        e.anatomy.impact_point = point;
        e.anatomy.impact_part = Some(part);
        e.anatomy.fracture = fracture_allowed && e.anatomy.impact_energy >= 72.;
        self.run.stats.damage_dealt += damage.min(e.hp);
        e.hp -= damage;
        e.hit = 0.18;
        e.anatomy.flinch[part as usize] = 0.3;
        e.anatomy.damage[part as usize] += structural;
        e.anatomy.impulse = dir * (3. + structural * 0.035).min(12.);
        let threshold = e.max_hp
            * match part {
                Part::Head => 0.32,
                Part::LeftArm | Part::RightArm => 0.28,
                _ => 0.34,
            };
        let sever = part != Part::Torso
            && !e.anatomy.missing(part)
            && (e.anatomy.damage[part as usize] >= threshold.clamp(18., 120.) || e.hp <= 0.);
        if sever {
            let mut detached = e.clone();
            detached.anatomy.death_pose = Some(Box::new(before.clone()));
            self.spawned_bodies.push(BodyEvent {
                enemy: detached,
                target: self.run.pos,
                part: Some(part),
                impulse: e.anatomy.impulse + Vec3::Y * 2.8,
                impact: point,
                energy: e.anatomy.impact_energy,
                fracture: e.anatomy.fracture && (e.hp <= 0. || (part == Part::Head && e.kind != 3)),
            });
            e.anatomy.severed |= 1 << part as usize;
            e.anatomy.stagger = 0.5;
            if part == Part::Head && e.kind != 3 {
                e.hp = 0.;
            }
            self.sound_events.push("bone");
        }
        if e.hp <= 0. {
            e.anatomy.death_pose = Some(Box::new(before));
        } else {
            e.pos = world_layout::move_body(e.pos, Vec3::new(dir.x, 0., dir.z) * 0.1, ENEMY_RADIUS);
        }
        self.floaters.push(Floater {
            pos: point + Vec3::Y * 0.1,
            text: if sever {
                part.label().into()
            } else if part == Part::Head {
                format!("{}  HEADSHOT", damage.round())
            } else {
                format!("{}", damage.round())
            },
            life: if sever { 1.1 } else { 0.7 },
        });
        for _ in 0..if sever { 18 } else { 8 } {
            let vel = dir * 1.8
                + Vec3::new(
                    (self.rand() - 0.5) * 3.,
                    self.rand() * 3.,
                    (self.rand() - 0.5) * 3.,
                );
            self.particles.push(Particle {
                pos: point,
                vel,
                life: 0.5,
                color: [0.8, 0.68, 0.42],
                size: 0.035,
            });
        }
        if self.run.weapon.major {
            self.run.hp = (self.run.hp + damage * 0.03).min(self.max_hp());
        }
    }
    pub(crate) fn collect_dead(&mut self) {
        for e in self.run.enemies.iter().filter(|e| e.hp <= 0.) {
            let recent_impact = self.run.time - e.anatomy.impact_time < 0.15;
            self.run.kills += 1;
            self.run.survival.orbs.push(crate::survival::Orb {
                pos: e.pos + Vec3::Y * 0.7,
                value: crate::encounters::species(e.kind).xp,
                age: 0.,
                attracted: false,
            });
            self.sound_events.push("bone");
            self.spawned_bodies.push(BodyEvent {
                enemy: e.clone(),
                target: self.run.pos,
                part: None,
                impulse: if recent_impact {
                    e.anatomy.impulse
                } else {
                    Vec3::ZERO
                },
                impact: if recent_impact {
                    e.anatomy.impact_point
                } else {
                    e.pos + Vec3::Y
                },
                energy: if recent_impact {
                    e.anatomy.impact_energy
                } else {
                    0.
                },
                fracture: recent_impact
                    && e.anatomy.fracture
                    && !(e.anatomy.impact_part == Some(Part::Head)
                        && e.anatomy.missing(Part::Head)),
            });
        }
        self.run.enemies.retain(|e| e.hp > 0.);
    }
    pub fn update(&mut self, dt: f32) {
        self.elapsed += dt;
        self.notice_time = (self.notice_time - dt).max(0.);
        self.update_tips(dt);
        if self.mode != Mode::Arena {
            return;
        }
        self.run.time += dt;
        if self.melee_pending {
            self.melee_timer -= dt;
            if self.melee_timer <= 0. {
                self.melee_pending = false;
                self.resolve_melee();
            }
        }

        self.spin = (self.spin + if self.input.fire { dt * 1.5 } else { -dt * 2. }).clamp(0., 1.);
        if self.burst_remaining > 0 {
            self.burst_timer -= dt;
            if self.burst_timer <= 0. {
                self.burst_remaining -= 1;
                self.burst_timer += 0.075;
                self.emit_shot();
            }
        }
        self.update_projectiles(dt);
        self.cooldown = (self.cooldown - dt).max(0.);
        self.flash = (self.flash - dt).max(0.);
        self.shot_age += dt;
        self.look_sway *= (-dt * 14.).exp();
        let speed = (self.input.forward.abs() + self.input.right.abs()).min(1.);
        self.motion_speed += (speed - self.motion_speed) * (1. - (-dt * 12.).exp());
        self.hurt = (self.hurt - dt).max(0.);
        if let Some(marker) = &mut self.hit_marker {
            marker.life -= dt;
        }
        self.hit_marker = self.hit_marker.filter(|m| m.life > 0.);
        for mark in &mut self.damage_marks {
            mark.life -= dt;
        }
        self.damage_marks.retain(|m| m.life > 0.);
        self.dash = (self.dash - dt).max(0.);
        self.dash_cd = (self.dash_cd - dt).max(0.);
        self.spell_cd = (self.spell_cd - dt).max(0.);
        if self.reload > 0. {
            let duration = self.run.weapon.reload_time();
            let before = 1. - self.reload / duration;
            self.reload = (self.reload - dt).max(0.);
            let after = 1. - self.reload / duration;
            for &(at, event) in crate::motion::reload_cues(self.run.weapon.kind) {
                if before < at && after >= at {
                    self.sound_events.push(event);
                }
            }
            if self.reload <= 0. {
                self.run.ammo = self.run.weapon.capacity();
            }
        }
        self.run.mana = (self.run.mana + dt * (4. + self.run.weapon.paths[2] as f32)).min(60.);
        let f = Vec3::new(self.run.yaw.sin(), 0., -self.run.yaw.cos());
        let r = Vec3::new(self.run.yaw.cos(), 0., self.run.yaw.sin());
        let mut movement = f * self.input.forward + r * self.input.right;
        if movement.length_squared() > 0. {
            movement = movement.normalize();
        } else if self.dash > 0. {
            movement = f;
        }
        let speed = if self.dash > 0. {
            19.
        } else if self.input.sprint {
            7.5
        } else {
            4.5
        };
        self.run.pos = world_layout::move_body(
            self.run.pos,
            movement * dt * speed * (1. + self.run.survival.ranks[9] as f32 * 0.08),
            PLAYER_RADIUS,
        );
        self.tick_enemies(dt);
        self.tick_powers(dt);
        self.tick_director(dt);
        self.tick_orbs(dt);
        for p in &mut self.particles {
            p.life -= dt;
            p.pos += p.vel * dt;
            p.vel.y -= dt * 3.;
        }
        self.particles.retain(|p| p.life > 0.);
        for f in &mut self.floaters {
            f.life -= dt;
            f.pos.y += dt * 0.7;
        }
        self.floaters.retain(|f| f.life > 0.);
        self.collect_dead();
        if self.input.fire {
            self.fire(false);
        }
        if self.practice_backup.is_some() {
            self.run.hp = 100.;
        }
        if self.run.hp <= 0. {
            self.run.hp = 0.;
            self.mode = Mode::Dead;
            self.track_records(None);
            self.save();
        } else if self.run.survival.pending > 0 {
            self.offer_power();
        } else if self.run.enemies.is_empty()
            && self.run.survival.remaining == 0
            && self.run.survival.orbs.is_empty()
        {
            self.complete_wave();
        }
    }
    pub fn complete_wave(&mut self) {
        if self.practice_backup.is_some() {
            self.start_wave();
            let back = self.back_prompt();
            self.notify(&format!("Fresh targets. {back} returns to the armory."));
            return;
        }
        if self.run.wave == crate::survival::DESCENTS && !self.run.survival.endless {
            self.mode = Mode::Victory;
            self.track_records(Some(self.run.time));
        } else {
            self.track_records(None);
            let reward = 85 + self.run.wave * 5;
            self.run.gold += reward;
            self.run.hp = (self.run.hp + 25.).min(self.max_hp());
            self.run.pack_buys = 0;
            self.run.draws = 0;
            self.run.offer = if self.run.wave == 1 {
                Card {
                    kind: WeaponKind::Double,
                    rarity: 2,
                    ..Card::starter()
                }
            } else {
                self.roll_card()
            };
            self.mode = Mode::Shop;
            self.notify(&format!("DESCENT CLEARED  +{reward} GOLD  /  +25 VITALITY"));
        }
        self.input = Input::default();
        self.save();
    }
    pub fn next_wave(&mut self) {
        self.run.wave += 1;
        self.start_wave();
        self.track_records(None);
    }
    pub fn spend(&mut self, cost: u32) -> bool {
        if self.run.gold < cost {
            self.notify("Not enough gold. The Collector does not extend credit.");
            self.sound_events.push("deny");
            false
        } else {
            self.run.gold -= cost;
            self.sound_events.push("coin");
            true
        }
    }
    pub fn equip(&mut self, card: Card) {
        if let Some(old) = self
            .run
            .inventory
            .iter_mut()
            .find(|c| c.kind == self.run.weapon.kind && c.rarity == self.run.weapon.rarity)
        {
            *old = self.run.weapon.clone();
        }
        self.run.weapon = card.clone();
        self.run.ammo = card.capacity();
        self.run.inventory.push(card);
        self.notify("A new pact is made. Weapon equipped.");
        self.save();
    }
    pub fn draw(&mut self) {
        let cost = 45 + self.run.draws * 10;
        if self.spend(cost) {
            self.run.draws += 1;
            let card = self.roll_card();
            self.equip(card);
            self.save();
        }
    }
    pub fn buy_offer(&mut self) {
        let cost = 35 + self.run.offer.rarity as u32 * 20;
        if self.spend(cost) {
            self.equip(self.run.offer.clone());
            self.run.offer = self.roll_card();
            self.save();
        }
    }
    pub fn open_pack(&mut self) {
        let cost = 30 + self.run.pack_buys * 15;
        if self.spend(cost) {
            self.run.pack_buys += 1;
            self.run.choices = (0..3).map(|_| self.roll_card()).collect();
            self.mode = Mode::Pack;
            self.save();
        }
    }
    pub fn select_pack(&mut self, i: usize) {
        if let Some(card) = self.run.choices.get(i).cloned() {
            self.mode = Mode::Shop;
            self.run.choices.clear();
            self.equip(card);
            self.save();
        }
    }
    pub fn upgrade(&mut self, path: usize) {
        if path >= 3 {
            return;
        }
        let level = self.run.weapon.paths[path];
        if level >= 5 {
            return;
        }
        let cost = 15 + level as u32 * 12;
        if self.spend(cost) {
            self.run.weapon.paths[path] += 1;
            self.notify("The pact deepens. Upgrade bound to this card.");
            self.save();
        }
    }
    pub fn ascend(&mut self) {
        if self.run.weapon.paths.contains(&5) && !self.run.weapon.major && self.spend(120) {
            self.run.weapon.major = true;
            self.notify("SOUL SIPHON — Damage restores vitality.");
            self.save();
        }
    }
    pub fn open_book(&mut self, mode: Mode) {
        self.return_mode = self.mode;
        self.mode = mode;
    }
    pub fn practice(&mut self, card: Card) {
        if self.practice_backup.is_some() {
            return;
        }
        self.practice_backup = Some(self.run.clone());
        self.run = Run::default();
        self.run.weapon = card;
        self.start_wave();
        let back = self.back_prompt().to_uppercase();
        self.notify(&format!(
            "PRACTICE GROUNDS  /  {back} returns to the armory. Your run is preserved."
        ));
    }
    pub fn back(&mut self) {
        if let Some(run) = self.practice_backup.take() {
            self.run = run;
            self.reset_effects();
            self.mode = Mode::Collection;
            return;
        }
        match self.mode {
            Mode::Collection | Mode::Bestiary => self.mode = self.return_mode,
            Mode::Tree => self.mode = Mode::Shop,
            Mode::Arena => {
                self.mode = Mode::Paused;
                self.input = Input::default();
                self.save();
            }
            Mode::Paused => self.mode = Mode::Arena,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chapel_cover_stops_bullets_and_projectiles_but_its_door_allows_fire() {
        for z in [-14., -5.] {
            let mut g = Game::new(false);
            g.mode = Mode::Arena;
            g.run.pos = Vec3::new(-17., 1.2, z);
            g.run.enemies = vec![Enemy::spawn(0, Vec3::new(-23., 0., z), 1, 0.)];
            let hp = g.run.enemies[0].hp;
            g.ray_attack(g.run.pos, -Vec3::X, WeaponKind::Pistol, 10.);
            assert_eq!(g.run.enemies[0].hp < hp, z == -5.);
            g.run.enemies[0].hp = hp;
            g.projectiles.push(Projectile {
                pos: g.run.pos,
                vel: -Vec3::X * 40.,
                life: 5.,
                damage: 10.,
                kind: WeaponKind::Crossbow,
            });
            g.update_projectiles(0.2);
            assert_eq!(g.run.enemies[0].hp < hp, z == -5.);
            assert!(g.projectiles.is_empty());
        }
    }
    #[test]
    fn legacy_foil_cards_load_as_standard_without_losing_upgrades() {
        let original = Card {
            kind: WeaponKind::Double,
            rarity: 3,
            paths: [2, 1, 3],
            major: true,
        };
        let mut legacy = serde_json::to_value(&original).unwrap();
        legacy["foil"] = serde_json::json!(true);
        let restored: Card = serde_json::from_value(legacy).unwrap();
        assert_eq!(restored.kind, original.kind);
        assert_eq!(restored.rarity, original.rarity);
        assert_eq!(restored.paths, original.paths);
        assert_eq!(restored.major, original.major);
        assert_eq!(restored.damage(), original.damage());
        assert!(
            serde_json::to_value(restored)
                .unwrap()
                .get("foil")
                .is_none()
        );
    }
    #[test]
    fn player_can_traverse_beyond_the_old_arena_and_stops_at_new_boundary() {
        let mut g = Game::new(false);
        g.mode = Mode::Arena;
        g.run.pos = Vec3::new(0., 1.65, 20.);
        g.run.survival.remaining = 100;
        g.run.survival.spawn_timer = 1000.;
        g.input.right = 1.;
        g.input.sprint = true;
        for _ in 0..400 {
            g.update(0.02);
        }
        assert!(g.run.pos.x > 40.);
        assert!(world_layout::is_walkable(g.run.pos, PLAYER_RADIUS));
        assert!(g.run.pos.x <= world_layout::HALF_EXTENT - PLAYER_RADIUS);
    }
    #[test]
    fn rebrand_preserves_saves_and_never_overwrites_newer_progress() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let support = std::env::temp_dir().join(format!("gravewake-migration-{stamp}"));
        let old = support.join("Dark Veil");
        let new = support.join("Gravewake");
        // Fresh installs should not create unnecessary directories.
        migrate_legacy_saves(&new, std::slice::from_ref(&old)).unwrap();
        assert!(!new.exists());
        std::fs::create_dir_all(&old).unwrap();
        let game = Game::new(false);
        let saved = serde_json::to_vec(&Save {
            version: 1,
            mode: Mode::Arena,
            run: game.run,
        })
        .unwrap();
        for (name, bytes) in [
            ("run.json", saved.as_slice()),
            ("graphics.json", b"0.65".as_slice()),
            ("performance.json", b"[false,true]".as_slice()),
        ] {
            std::fs::write(old.join(name), bytes).unwrap();
        }
        migrate_legacy_saves(&new, std::slice::from_ref(&old)).unwrap();
        for name in ["run.json", "graphics.json", "performance.json"] {
            assert_eq!(
                std::fs::read(old.join(name)).unwrap(),
                std::fs::read(new.join(name)).unwrap()
            );
        }
        let restored: Save =
            serde_json::from_slice(&std::fs::read(new.join("run.json")).unwrap()).unwrap();
        assert_eq!(restored.version, 1);
        assert_eq!(restored.mode, Mode::Arena);
        std::fs::write(new.join("run.json"), b"newer progress").unwrap();
        std::fs::write(new.join("graphics.json"), b"0.8").unwrap();
        migrate_legacy_saves(&new, std::slice::from_ref(&old)).unwrap();
        assert_eq!(
            std::fs::read(new.join("run.json")).unwrap(),
            b"newer progress"
        );
        assert_eq!(std::fs::read(new.join("graphics.json")).unwrap(), b"0.8");
        assert_eq!(std::fs::read(old.join("run.json")).unwrap(), saved);
        std::fs::remove_dir_all(support).unwrap();
    }
    #[test]
    fn player_runs_are_seeded_freshly_while_diagnostics_stay_fixed() {
        // Opening state that the seed controls: spawn positions and a pack roll.
        let opening = |g: &mut Game| {
            let spawns: Vec<_> = g.run.enemies.iter().map(|e| e.pos.to_array()).collect();
            let card = g.roll_card();
            (spawns, card.kind, card.rarity)
        };
        let mut fixed = Game::new(false);
        fixed.new_run();
        let first = opening(&mut fixed);
        fixed.new_run();
        assert_eq!(opening(&mut fixed), first);

        // Use the player seed source without enabling real save writes.
        let mut player = Game::new(false);
        let mut seeds = std::collections::HashSet::new();
        let mut openings = vec![];
        for _ in 0..4 {
            let seed = fresh_seed();
            seeds.insert(seed);
            player.new_run_with_seed(seed);
            openings.push(opening(&mut player));
        }
        assert_eq!(seeds.len(), 4);
        assert!(openings.windows(2).all(|w| w[0].0 != w[1].0));
    }
    #[test]
    fn preferences_roundtrip_and_tolerate_old_or_bad_values() {
        let custom = Preferences {
            sensitivity: 0.004,
            volume: 0.8,
            fov: 85.,
            invert_y: true,
            reduce_flashes: true,
            ..Preferences::default()
        };
        let bytes = serde_json::to_vec(&custom).unwrap();
        assert_eq!(Preferences::from_json(&bytes), Some(custom));
        // Fields added later fall back to defaults instead of discarding the file.
        let partial = Preferences::from_json(br#"{"volume":0.2}"#).unwrap();
        assert_eq!(partial.volume, 0.2);
        assert_eq!(partial.fov, Preferences::default().fov);
        assert!(!partial.invert_y);
        let clamped =
            Preferences::from_json(br#"{"sensitivity":1.0,"volume":-3,"fov":200}"#).unwrap();
        assert_eq!(clamped.sensitivity, Preferences::SENSITIVITY_RANGE.1);
        assert_eq!(clamped.volume, 0.);
        assert_eq!(clamped.fov, Preferences::FOV_RANGE.1);
        let nan = Preferences {
            fov: f32::NAN,
            ..custom
        }
        .sanitized();
        assert_eq!(nan.fov, Preferences::default().fov);
        assert_eq!(Preferences::from_json(b"not json"), None);
        // A damaged bindings entry resets only the bindings.
        let damaged =
            Preferences::from_json(br#"{"volume":0.3,"bindings":[{"action":"Nope"}]}"#).unwrap();
        assert_eq!(damaged.volume, 0.3);
        assert_eq!(damaged.bindings, Bindings::default());
    }
    #[test]
    fn rebinding_waits_for_a_usable_key_and_reports_swaps() {
        use winit::keyboard::KeyCode;
        let mut g = Game::new(false);
        g.bind(Trigger::Key(KeyCode::KeyF), None);
        assert_eq!(g.prefs.bindings, Bindings::default(), "nothing is waiting");
        g.rebinding = Some(Action::Reload);
        g.bind(Trigger::Key(KeyCode::F7), None);
        assert_eq!(g.rebinding, Some(Action::Reload));
        assert!(g.controls_note.contains("F7 is reserved"), "{}", g.controls_note);
        g.bind(Trigger::Key(KeyCode::KeyE), Some('e'));
        assert_eq!(g.rebinding, None);
        assert_eq!(g.controls_note, "Reload is now E. Melee moved to R.");
        assert_eq!(
            g.prefs.bindings.action(Trigger::Key(KeyCode::KeyR)),
            Some(Action::Melee)
        );
    }
    #[test]
    fn save_directories_follow_platform_conventions() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| std::ffi::OsString::from(v))
            }
        };
        let (current, legacy) = save_dirs(env(&[("HOME", "/home/hunter")]));
        if cfg!(target_os = "macos") {
            assert_eq!(
                current,
                Path::new("/home/hunter/Library/Application Support/Gravewake")
            );
            assert_eq!(
                legacy,
                [Path::new(
                    "/home/hunter/Library/Application Support/Dark Veil"
                )]
            );
        } else if cfg!(target_os = "linux") {
            assert_eq!(current, Path::new("/home/hunter/.local/share/gravewake"));
            assert_eq!(
                legacy,
                [
                    Path::new("/home/hunter/Library/Application Support/Gravewake"),
                    Path::new("/home/hunter/Library/Application Support/Dark Veil"),
                ]
            );
            let (xdg, _) = save_dirs(env(&[("HOME", "/home/hunter"), ("XDG_DATA_HOME", "/data")]));
            assert_eq!(xdg, Path::new("/data/gravewake"));
            // The XDG spec says relative values are invalid and must be ignored.
            let (relative, _) =
                save_dirs(env(&[("HOME", "/home/hunter"), ("XDG_DATA_HOME", "data")]));
            assert_eq!(relative, Path::new("/home/hunter/.local/share/gravewake"));
        }
    }
    #[test]
    fn migration_prefers_the_newest_legacy_location() {
        let support = quit_save_test_directory("legacy-priority");
        let current = support.join("gravewake");
        let recent = support.join("Library/Application Support/Gravewake");
        let oldest = support.join("Library/Application Support/Dark Veil");
        std::fs::create_dir_all(&recent).unwrap();
        std::fs::create_dir_all(&oldest).unwrap();
        std::fs::write(recent.join("run.json"), b"recent run").unwrap();
        std::fs::write(oldest.join("run.json"), b"oldest run").unwrap();
        std::fs::write(oldest.join("graphics.json"), b"0.5").unwrap();
        migrate_legacy_saves(&current, &[recent.clone(), oldest.clone()]).unwrap();
        assert_eq!(
            std::fs::read(current.join("run.json")).unwrap(),
            b"recent run"
        );
        assert_eq!(
            std::fs::read(current.join("graphics.json")).unwrap(),
            b"0.5"
        );
        assert!(!current.join("performance.json").exists());
        assert_eq!(
            std::fs::read(recent.join("run.json")).unwrap(),
            b"recent run"
        );
        std::fs::remove_dir_all(support).unwrap();
    }
    fn quit_save_test_directory(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "gravewake-quit-save-{label}-{}-{stamp}",
            std::process::id()
        ))
    }
    #[test]
    fn quit_save_paused_run_restores_arena_progress() {
        let directory = quit_save_test_directory("paused");
        let path = directory.join("run.json");
        let mut game = Game::new(false);
        game.new_run();
        game.mode = Mode::Paused;
        game.run.gold = 173;
        game.run.hp = 61.;
        game.run.kills = 19;
        game.run.pos = Vec3::new(2., 1.65, -7.);
        let expected_run = serde_json::to_value(&game.run).unwrap();
        game.has_save = false;
        game.save_enabled = true;

        assert!(game.save_to(&path));
        assert!(game.has_save);
        assert_eq!(game.mode, Mode::Paused);
        let restored: Save = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(restored.version, 1);
        assert_eq!(restored.mode, Mode::Arena);
        assert_eq!(serde_json::to_value(restored.run).unwrap(), expected_run);
        assert!(!path.with_extension("tmp").exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn quit_save_write_failure_returns_false_and_reports_error() {
        let directory = quit_save_test_directory("failure");
        std::fs::create_dir_all(&directory).unwrap();
        // A file where the parent directory belongs reliably fails even as root.
        let blocked_parent = directory.join("blocked");
        std::fs::write(&blocked_parent, b"keep this file").unwrap();
        let mut game = Game::new(false);
        game.mode = Mode::Paused;
        game.has_save = false;
        game.save_enabled = true;

        assert!(!game.save_to(&blocked_parent.join("run.json")));
        assert!(game.notice.starts_with("Could not save:"));
        assert!(game.notice_time > 0.);
        assert!(!game.has_save);
        assert_eq!(game.mode, Mode::Paused);
        assert_eq!(std::fs::read(blocked_parent).unwrap(), b"keep this file");
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn quit_save_title_and_practice_preserve_existing_run() {
        let directory = quit_save_test_directory("preserve");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("run.json");
        let original = b"existing saved progress";
        std::fs::write(&path, original).unwrap();
        let mut game = Game::new(false);
        game.save_enabled = true;

        assert!(game.save_to(&path));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        game.mode = Mode::Collection;
        game.return_mode = Mode::Title;
        assert!(game.save_to(&path));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        game.practice_backup = Some(game.run.clone());
        game.mode = Mode::Arena;
        game.run.gold = 999;
        assert!(game.save_to(&path));
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!path.with_extension("tmp").exists());
        std::fs::remove_dir_all(directory).unwrap();
    }
    fn target(x: f32, z: f32) -> Enemy {
        Enemy {
            pos: Vec3::new(x, 0., z),
            hp: 500.,
            max_hp: 500.,
            kind: 0,
            attack: 9.,
            phase: 0.,
            hit: 0.,
            burn: 0.,
            poison: 0.,
            slow: 0.,
            anatomy: Anatomy::default(),
            ai: crate::encounters::Brain::default(),
        }
    }
    #[test]
    fn fatal_impacts_preserve_the_visible_pose_and_exact_hit_point() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.enemies = vec![target(2., 0.)];
        g.run.enemies[0].hp = 20.;
        g.run.enemies[0].phase = 0.37;
        g.run.enemies[0].hit = 0.04;
        let before = Pose::for_enemy(&g.run.enemies[0], g.run.pos);
        let point = before.root.transform_point3(Vec3::new(0.1, 1.25, 0.));
        g.apply_hit_at(0, 90., WeaponKind::HandCannon, -Vec3::Z, Part::Torso, point);
        g.collect_dead();
        let event = g.spawned_bodies.iter().find(|e| e.part.is_none()).unwrap();
        let pose = event.enemy.anatomy.death_pose.as_ref().unwrap();
        assert_eq!(
            pose.root, before.root,
            "fatal knockback/flinch cannot teleport the ragdoll"
        );
        assert_eq!(event.impact, point);
        assert_eq!(event.energy, 90.);
        assert!(event.fracture);
        assert_eq!(g.run.kills, 1);
        assert_eq!(g.run.survival.orbs.len(), 1);
        g.collect_dead();
        assert_eq!(g.run.survival.orbs.len(), 1);
    }
    #[test]
    fn shotgun_energy_accumulates_within_one_impact_but_not_across_old_hits() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.enemies = vec![target(0., 0.)];
        g.run.enemies[0].hp = 80.;
        let first_visible = Pose::for_enemy(&g.run.enemies[0], g.run.pos);
        let point = Vec3::Y * 1.2;
        for _ in 0..5 {
            g.apply_hit_at(0, 16., WeaponKind::Double, -Vec3::Z, Part::Torso, point);
        }
        g.collect_dead();
        assert!(
            g.spawned_bodies
                .iter()
                .any(|e| e.fracture && e.energy == 80.)
        );
        assert_eq!(
            g.spawned_bodies[0]
                .enemy
                .anatomy
                .death_pose
                .as_ref()
                .unwrap()
                .root,
            first_visible.root,
            "one shotgun volley must hand off the original visible pose"
        );
        g.spawned_bodies.clear();
        g.run.enemies = vec![target(0., 0.)];
        g.run.enemies[0].hp = 80.;
        for _ in 0..5 {
            g.run.time += 0.7;
            g.apply_hit_at(0, 16., WeaponKind::Double, -Vec3::Z, Part::Torso, point);
        }
        g.collect_dead();
        assert!(
            g.spawned_bodies
                .iter()
                .all(|e| !e.fracture && e.energy == 16.)
        );
    }
    #[test]
    fn detached_limb_keeps_preimpact_pose_and_transient_physics_does_not_enter_saves() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.enemies = vec![target(0., 0.)];
        g.run.enemies[0].hp = 250.;
        g.run.enemies[0].max_hp = 100.;
        let before = Pose::for_enemy(&g.run.enemies[0], g.run.pos);
        let point = before.anchor(Part::LeftArm);
        g.apply_hit_at(0, 40., WeaponKind::Pistol, -Vec3::Z, Part::LeftArm, point);
        let event = &g.spawned_bodies[0];
        assert_eq!(
            event.enemy.anatomy.death_pose.as_ref().unwrap().root,
            before.root
        );
        assert_eq!(event.part, Some(Part::LeftArm));
        assert!(g.run.enemies[0].anatomy.death_pose.is_none());
        let json = serde_json::to_string(&g.run.enemies[0]).unwrap();
        assert!(!json.contains("death_pose") && !json.contains("impact_energy"));
        let restored: Enemy = serde_json::from_str(&json).unwrap();
        assert!(restored.anatomy.missing(Part::LeftArm));
        assert_eq!(restored.anatomy.impact_energy, 0.);
    }
    #[test]
    fn corpse_shot_impulses_stop_at_cover_and_do_not_award_gameplay_progress() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.enemies.clear();
        let origin = Vec3::new(-17., 1., -14.);
        g.ray_attack(origin, -Vec3::X, WeaponKind::Pistol, 30.);
        match g.physics_impacts.last().unwrap() {
            anatomy::PhysicsImpact::Ray { range, energy, .. } => {
                assert!(*range < 3. && *range > 2.);
                assert_eq!(*energy, 30.);
            }
            _ => panic!("expected bullet impulse"),
        }
        assert_eq!(g.run.kills, 0);
        assert!(g.run.survival.orbs.is_empty());
    }
    #[test]
    fn repeated_pellet_rays_hit_the_visible_pose_until_the_next_frame() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 4.);
        g.run.enemies = vec![target(0., 0.)];
        let pose = Pose::for_enemy(&g.run.enemies[0], g.run.pos);
        let aim = pose.anchor(Part::Torso);
        let direction = (aim - g.run.pos).normalize();
        let (part, distance) = anatomy::trace(
            &g.run.enemies[0],
            g.run.pos,
            g.run.pos,
            direction,
            20.,
            g.run.time,
        )
        .unwrap();
        assert_eq!(part, Part::Torso);
        let visible_impact = g.run.pos + direction * distance;
        for _ in 0..8 {
            g.ray_attack(g.run.pos, direction, WeaponKind::Coachgun, 16.);
            assert!(
                g.run.enemies[0]
                    .anatomy
                    .impact_point
                    .distance(visible_impact)
                    < 0.0001
            );
        }
        assert!((g.run.enemies[0].hp - (500. - 128.)).abs() < 0.001);
        let moved = anatomy::trace(
            &g.run.enemies[0],
            g.run.pos,
            g.run.pos,
            direction,
            20.,
            g.run.time + 1. / 60.,
        );
        assert!(
            moved.is_none_or(|(_, t)| (t - distance).abs() > 0.1),
            "the next frame must use the moved enemy rather than the cached volley pose"
        );
    }
    #[test]
    fn a_heavy_headshot_fractures_the_detached_skull_not_the_unhit_torso() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.enemies = vec![target(0., 0.)];
        let point = Pose::for_enemy(&g.run.enemies[0], g.run.pos).anchor(Part::Head);
        g.apply_hit_at(0, 140., WeaponKind::HandCannon, -Vec3::Z, Part::Head, point);
        g.collect_dead();
        assert_eq!(g.spawned_bodies.len(), 2);
        let skull = g
            .spawned_bodies
            .iter()
            .find(|e| e.part == Some(Part::Head))
            .unwrap();
        assert!(skull.fracture);
        let corpse = g.spawned_bodies.iter().find(|e| e.part.is_none()).unwrap();
        assert!(!corpse.fracture && corpse.enemy.anatomy.missing(Part::Head));
    }
    #[test]
    fn anatomical_hits_sever_only_the_target_and_survive_saves() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 4.);
        let mut enemy = target(0., 0.);
        enemy.hp = 100.;
        enemy.max_hp = 100.;
        g.run.enemies = vec![enemy];
        let pose = Pose::for_enemy(&g.run.enemies[0], g.run.pos);
        let arm = pose.arms[0];
        let point = pose.root.transform_point3((arm.root + arm.joint) * 0.5);
        let origin = Vec3::new(point.x, point.y, 4.);
        let (part, _) = anatomy::trace(
            &g.run.enemies[0],
            g.run.pos,
            origin,
            -Vec3::Z,
            20.,
            g.run.time,
        )
        .unwrap();
        assert_eq!(part, Part::LeftArm);
        g.ray_attack(origin, -Vec3::Z, WeaponKind::Pistol, 40.);
        assert!(g.run.enemies[0].hp > 0.);
        assert!(g.run.enemies[0].anatomy.missing(Part::LeftArm));
        assert!(!g.run.enemies[0].anatomy.missing(Part::RightArm));
        assert_eq!(g.spawned_bodies.len(), 1);
        assert_eq!(g.spawned_bodies[0].part, Some(Part::LeftArm));
        let hp = g.run.enemies[0].hp;
        g.ray_attack(origin, -Vec3::Z, WeaponKind::Pistol, 40.);
        assert_eq!(
            g.run.enemies[0].hp, hp,
            "missing arm must not remain a hit target"
        );
        let restored: Enemy =
            serde_json::from_str(&serde_json::to_string(&g.run.enemies[0]).unwrap()).unwrap();
        assert!(restored.anatomy.missing(Part::LeftArm));
        let mut legacy = serde_json::to_value(&restored).unwrap();
        legacy.as_object_mut().unwrap().remove("anatomy");
        let legacy: Enemy = serde_json::from_value(legacy).unwrap();
        assert_eq!(legacy.anatomy.severed, 0, "old saves gain intact anatomy");
    }
    #[test]
    fn headshot_detaches_one_skull_and_legs_change_movement() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 4.);
        let mut e = target(0., 0.);
        e.hp = 100.;
        e.max_hp = 100.;
        g.run.enemies = vec![e.clone()];
        g.ray_attack(Vec3::new(0., 1.74, 4.), -Vec3::Z, WeaponKind::Pistol, 40.);
        g.collect_dead();
        g.collect_dead();
        assert_eq!(g.run.kills, 1);
        assert_eq!(
            g.spawned_bodies
                .iter()
                .filter(|e| e.part == Some(Part::Head))
                .count(),
            1
        );
        let corpse = g.spawned_bodies.iter().find(|e| e.part.is_none()).unwrap();
        assert!(corpse.enemy.anatomy.missing(Part::Head));
        g.run.enemies = vec![e];
        for part in [Part::LeftLeg, Part::RightLeg] {
            let p = Pose::for_enemy(&g.run.enemies[0], g.run.pos).anchor(part);
            g.apply_hit_at(0, 35., WeaponKind::Pistol, -Vec3::Z, part, p);
        }
        assert!(g.run.enemies[0].hp > 0.);
        assert_eq!(g.run.enemies[0].anatomy.legs(), 2);
        assert_eq!(g.run.enemies[0].anatomy.speed(), 0.2);
        g.update(0.35);
        let pose = Pose::for_enemy(&g.run.enemies[0], g.run.pos);
        assert!(
            pose.root.transform_point3(Vec3::new(0., 1.72, 0.)).y < 0.8,
            "both legs lost lowers the enemy into a crawl"
        );
    }
    #[test]
    fn animated_scaled_and_floating_targets_use_their_visible_head() {
        for kind in 0..crate::encounters::ROSTER.len() {
            for phase in [0., 0.37, 1.6] {
                let mut e = target(0., 0.);
                e.kind = kind;
                e.phase = phase;
                let target = Vec3::new(3., 1.65, 4.);
                let pose = Pose::for_enemy(&e, target);
                let head = pose.root.transform_point3(Vec3::new(0., pose.head_y, 0.));
                let dir = (e.pos - target) * Vec3::new(1., 0., 1.);
                let dir = dir.normalize();
                let hit = anatomy::trace(&e, target, head - dir * 4., dir, 8., 0.).unwrap();
                assert_eq!(hit.0, Part::Head, "kind {kind}, phase {phase}");
                assert!(
                    anatomy::trace(&e, target, head - dir * 4. + Vec3::Y * 2., dir, 8., 0.)
                        .is_none()
                );
            }
        }
    }
    #[test]
    fn severed_arms_reduce_attack_strength_and_lost_legs_reduce_speed() {
        let run = |severed: u8, near: bool| {
            let mut g = Game::new(false);
            g.new_run();
            g.run.armor = 0.;
            g.run.pos = Vec3::new(0., 1.65, if near { 1. } else { 5. });
            let mut e = target(0., 0.);
            e.anatomy.severed = severed;
            e.attack = if near { 0. } else { 10. };
            g.run.enemies = vec![e];
            g.update(0.1);
            (g.run.hp, g.run.enemies[0].pos.z)
        };
        assert!(run((1 << 2) | (1 << 3), true).0 > run(0, true).0);
        assert!(run(1 << 4, false).1 < run(0, false).1 * 0.6);
        assert!(run((1 << 4) | (1 << 5), false).1 < run(0, false).1 * 0.25);
    }
    #[test]
    fn fast_projectiles_keep_head_hits_and_nearest_targets_occlude() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 4.);
        let mut front = target(0., 0.);
        front.hp = 100.;
        front.max_hp = 100.;
        let mut back = front.clone();
        back.pos.z = -2.;
        g.run.enemies = vec![front.clone(), back];
        g.ray_attack(Vec3::new(0., 1.2, 4.), -Vec3::Z, WeaponKind::Pistol, 20.);
        assert!(g.run.enemies[0].hp < 100.);
        assert_eq!(g.run.enemies[1].hp, 100.);
        for kind in [WeaponKind::Crossbow, WeaponKind::EmberStaff] {
            g.run.enemies = vec![front.clone()];
            g.spawned_bodies.clear();
            g.projectiles = vec![Projectile {
                pos: Vec3::new(0., 1.74, 3.),
                vel: -Vec3::Z * 100.,
                life: 2.,
                damage: 50.,
                kind,
            }];
            g.update_projectiles(0.05);
            assert!(g.run.enemies.is_empty(), "{kind:?} swept through skull");
            assert!(
                g.spawned_bodies.iter().any(|e| e.part == Some(Part::Head)),
                "{kind:?} retained impact location"
            );
        }
    }
    #[test]
    fn weapon_hits_set_reticle_markers_and_kills_tick_once() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 4.);
        let mut front = target(0., 0.);
        front.hp = 100.;
        front.max_hp = 100.;
        let kind = |g: &Game| g.hit_marker.map(|m| m.kind);
        // Bullet to the torso.
        g.run.enemies = vec![front.clone()];
        g.ray_attack(Vec3::new(0., 1.2, 4.), -Vec3::Z, WeaponKind::Pistol, 20.);
        assert_eq!(kind(&g), Some(HitKind::Body));
        // A light projectile to the skull, then a lethal one.
        for (damage, expected) in [(8., HitKind::Head), (50., HitKind::Kill)] {
            g.hit_marker = None;
            g.run.enemies = vec![front.clone()];
            g.projectiles = vec![Projectile {
                pos: Vec3::new(0., 1.74, 3.),
                vel: -Vec3::Z * 100.,
                life: 2.,
                damage,
                kind: WeaponKind::Crossbow,
            }];
            g.update_projectiles(0.05);
            assert_eq!(kind(&g), Some(expected), "{damage} damage projectile");
        }
        // Two pellets killing two enemies in the same update tick once.
        g.hit_marker = None;
        g.sound_events.clear();
        let mut second = front.clone();
        second.pos.x = 1.;
        g.run.enemies = vec![front.clone(), second];
        for i in 0..2 {
            let point = Pose::for_enemy(&g.run.enemies[i], g.run.pos).anchor(Part::Torso);
            g.apply_hit_at(i, 200., WeaponKind::Double, -Vec3::Z, Part::Torso, point);
        }
        assert_eq!(kind(&g), Some(HitKind::Kill));
        assert_eq!(g.sound_events.iter().filter(|e| **e == "kill").count(), 1);
        // A melee weapon strike and the E bash both register.
        g.reset_effects();
        g.run.pos = Vec3::new(0., 1.65, 0.);
        g.run.weapon.kind = WeaponKind::Cleaver;
        g.run.enemies = vec![target(0., -2.)];
        g.fire(false);
        while g.run.enemies[0].hp == 500. && g.run.time < 1. {
            g.update(0.01);
        }
        // Melee picks the region nearest the aim; level eyes meet the skull.
        assert_eq!(kind(&g), Some(HitKind::Head));
        g.reset_effects();
        g.run.weapon.kind = WeaponKind::Pistol;
        g.run.enemies = vec![target(0., -2.)];
        g.melee();
        assert_eq!(kind(&g), Some(HitKind::Body));
        // Automatic powers are not the player's aim and leave the reticle alone.
        g.reset_effects();
        g.damage_enemy(0, 20., -Vec3::Y);
        assert_eq!(kind(&g), None);
    }
    #[test]
    fn sustained_dps_counts_pellets_magazines_bursts_and_upgrades() {
        let card = |kind, rarity, paths| Card {
            kind,
            rarity,
            paths,
            major: false,
        };
        let pistol = card(WeaponKind::Pistol, 0, [0; 3]);
        let s = WeaponKind::Pistol.spec();
        let expected = s.capacity as f32 * s.damage / (s.capacity as f32 * s.interval + s.reload);
        assert!((pistol.sustained_dps() - expected).abs() < 1e-3);
        let double = card(WeaponKind::Double, 0, [0; 3]);
        let d = WeaponKind::Double.spec();
        let expected = d.capacity as f32 * d.damage * d.pellets as f32
            / (d.capacity as f32 * d.interval + d.reload);
        assert!((double.sustained_dps() - expected).abs() < 1e-3);
        let cleaver = card(WeaponKind::Cleaver, 0, [0; 3]);
        let c = WeaponKind::Cleaver.spec();
        assert!((cleaver.sustained_dps() - c.damage / c.interval).abs() < 1e-3);
        // A three-round burst spends its magazine in a third as many pulls.
        let burst = card(WeaponKind::Carbine, 0, [0; 3]);
        let b = WeaponKind::Carbine.spec();
        assert_eq!(b.burst, 3);
        let pulls = b.capacity.div_ceil(3) as f32;
        let expected = b.capacity as f32 * b.damage / (pulls * b.interval + b.reload);
        assert!((burst.sustained_dps() - expected).abs() < 1e-3);
        // Rarity and both damage and speed upgrades raise the estimate.
        assert!(card(WeaponKind::Pistol, 2, [0; 3]).sustained_dps() > pistol.sustained_dps());
        assert!(card(WeaponKind::Pistol, 0, [2, 0, 0]).sustained_dps() > pistol.sustained_dps());
        assert!(card(WeaponKind::Pistol, 0, [0, 2, 0]).sustained_dps() > pistol.sustained_dps());
        for kind in WeaponKind::ALL {
            let dps = card(kind, 0, [0; 3]).sustained_dps();
            assert!(dps.is_finite() && dps > 0., "{kind:?}");
        }
    }
    #[test]
    fn shuffled_waves_keep_each_species_share_but_vary_the_order() {
        let order = |seed: u64, wave: u32, count: usize| {
            let mut g = Game::new(false);
            g.new_run_with_seed(seed);
            g.run.wave = wave;
            g.run.enemies.clear();
            g.run.survival.bag.clear();
            g.run.survival.spawned = 0;
            g.run.survival.remaining = 200;
            for _ in 0..count {
                g.spawn_reinforcement();
            }
            g.run.enemies.iter().map(|e| e.kind).collect::<Vec<_>>()
        };
        // Two full passes through descent 3's eight-species pool.
        let kinds = order(7, 3, 16);
        for species in [0, 1, 2, 4, 5, 6, 7, 8] {
            assert_eq!(
                kinds.iter().filter(|k| **k == species).count(),
                2,
                "{species}"
            );
        }
        assert_eq!(order(7, 3, 16), kinds, "one seed repeats its order");
        let others: Vec<_> = (8..12).map(|seed| order(seed, 3, 16)).collect();
        assert!(others.iter().any(|o| *o != kinds), "seeds vary the order");
        // Tithekeeper descents still open with the boss.
        assert_eq!(order(7, 4, 1), vec![3]);
        assert_eq!(order(99, 8, 1), vec![3]);
    }
    #[test]
    fn records_track_runs_depth_souls_and_victories() {
        let mut r = Records::default();
        assert_eq!(r.record(3, 40, None), vec!["DEEPEST DESCENT", "MOST SOULS"]);
        assert!(r.record(2, 10, None).is_empty());
        assert_eq!(
            r.record(12, 300, Some(1500.)),
            vec!["DEEPEST DESCENT", "MOST SOULS", "FASTEST VICTORY"]
        );
        assert!(r.record(12, 200, Some(1600.)).is_empty());
        assert_eq!(r.victories, 2);
        assert_eq!(r.fastest_victory, Some(1500.));
        let json = serde_json::to_vec(&r).unwrap();
        assert_eq!(serde_json::from_slice::<Records>(&json).unwrap(), r);
        let partial: Records = serde_json::from_slice(br#"{"deepest":4}"#).unwrap();
        assert_eq!(
            (partial.deepest, partial.runs, partial.fastest_victory),
            (4, 0, None)
        );

        // Through the game: a run, two descents, then death.
        let mut g = Game::new(false);
        g.new_run();
        assert_eq!((g.records.runs, g.records.deepest), (1, 1));
        g.run.kills = 25;
        g.mode = Mode::Shop;
        g.next_wave();
        assert_eq!(g.records.deepest, 2);
        g.run.hp = -1.;
        g.mode = Mode::Arena;
        g.update(0.01);
        assert_eq!(g.mode, Mode::Dead);
        assert_eq!(g.records.most_souls, 25);
        assert!(
            g.run_records.contains(&"DEEPEST DESCENT") && g.run_records.contains(&"MOST SOULS")
        );
        // A second, shallower run sets nothing new.
        g.new_run();
        assert_eq!(g.records.runs, 2);
        assert!(g.run_records.is_empty());
        // Victory on the twelfth descent records the time.
        g.run.wave = crate::survival::DESCENTS;
        g.run.time = 1234.;
        g.run.enemies.clear();
        g.run.survival.remaining = 0;
        g.run.survival.orbs.clear();
        g.complete_wave();
        assert_eq!(g.mode, Mode::Victory);
        assert_eq!(
            (g.records.victories, g.records.fastest_victory),
            (1, Some(1234.))
        );
        assert!(g.run_records.contains(&"FASTEST VICTORY"));
        // Records are written atomically and read back.
        let directory = quit_save_test_directory("records");
        let path = directory.join("records.json");
        g.write_records(&path).unwrap();
        let stored: Records = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored, g.records);
        assert!(!path.with_extension("tmp").exists());
        std::fs::remove_dir_all(directory).unwrap();
        // Practice never counts.
        let before = g.records;
        g.practice_backup = Some(g.run.clone());
        g.run.wave = 40;
        g.next_wave();
        assert_eq!(g.records, before);
    }
    #[test]
    fn special_attack_warnings_emit_a_positional_cue() {
        for (kind, cue) in [
            (2, "warn_cast"),
            (7, "warn_slam"),
            (4, "warn_dive"),
            (10, "warn_summon"),
            (11, "warn_blink"),
        ] {
            let mut g = Game::new(false);
            g.new_run();
            g.run.pos = Vec3::new(0., 1.65, 0.);
            // Reapers only blink from beyond four metres; vessels burst within five.
            let z = if kind == 11 { -7. } else { -3. };
            let mut e = crate::game::Enemy::spawn(kind, Vec3::new(2., 0., z), 1, 0.);
            e.ai.cooldown = 0.;
            let start = e.pos;
            g.run.enemies = vec![e];
            g.world_sounds.clear();
            g.tick_enemies(0.01);
            assert!(g.run.enemies[0].ai.warning > 0., "{kind} began its warning");
            let (event, pos) = g.world_sounds[0];
            assert_eq!(event, cue, "kind {kind}");
            // The cue sounds where the wind-up began, even if the enemy then moves.
            let gap = pos.distance(start + Vec3::Y);
            assert!(gap < 0.2, "kind {kind} cue {gap} m from the enemy");
        }
    }
    #[test]
    fn damage_arcs_point_toward_their_source_and_merge() {
        use std::f32::consts::{FRAC_PI_2, PI};
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 0.);
        let relative = |g: &Game| {
            let a = g.damage_marks[0].bearing - g.run.yaw;
            a.sin().atan2(a.cos())
        };
        // Facing -Z: front, right, behind and left of the player.
        for (yaw, x, z, expected) in [
            (0., 0., -5., 0.),
            (0., 5., 0., FRAC_PI_2),
            (0., 0., 5., PI),
            (0., -5., 0., -FRAC_PI_2),
            (FRAC_PI_2, 5., 0., 0.),
            (FRAC_PI_2, 0., 5., FRAC_PI_2),
        ] {
            g.damage_marks.clear();
            g.run.yaw = yaw;
            g.mark_damage_from(Vec3::new(x, 0., z));
            let r = relative(&g);
            assert!(
                (r.sin() - expected.sin()).abs() < 1e-4 && (r.cos() - expected.cos()).abs() < 1e-4,
                "source ({x}, {z}) at yaw {yaw}: {r}"
            );
        }
        g.damage_marks.clear();
        g.mark_damage_from(Vec3::new(5., 0., 0.));
        g.mark_damage_from(Vec3::new(5., 0., 0.5));
        assert_eq!(g.damage_marks.len(), 1);
        for i in 0..6 {
            let a = i as f32 * 1.05;
            g.mark_damage_from(Vec3::new(a.sin() * 5., 0., a.cos() * 5.));
        }
        assert_eq!(g.damage_marks.len(), MAX_DAMAGE_MARKS);
        g.mark_damage_from(g.run.pos);
        assert_eq!(g.damage_marks.len(), MAX_DAMAGE_MARKS);
        // A real enemy strike from the player's right marks that side.
        g.reset_effects();
        g.run.yaw = 0.;
        let mut attacker = target(1.2, 0.);
        attacker.attack = 0.;
        g.run.enemies = vec![attacker];
        let hp = g.run.hp;
        while g.run.hp == hp && g.run.time < 3. {
            g.update(0.01);
        }
        assert!(g.run.hp < hp, "the adjacent enemy should strike");
        assert_eq!(g.damage_marks.len(), 1);
        assert!(relative(&g) > 0.8 && relative(&g) < 2.4, "{}", relative(&g));
    }
    #[test]
    fn unseen_wind_ups_aimed_at_the_player_get_pointers() {
        use std::f32::consts::{FRAC_PI_2, PI};
        let half_view = 0.9;
        let setup = |kind: usize, x: f32, z: f32| {
            let mut g = Game::new(false);
            g.new_run();
            g.run.pos = Vec3::new(0., 1.65, 0.);
            g.run.yaw = 0.;
            let mut e = Enemy::spawn(kind, Vec3::new(x, 0., z), 1, 0.);
            e.ai.cooldown = 0.;
            g.run.enemies = vec![e];
            g.tick_enemies(0.01);
            g
        };
        // Behind the player (facing -Z): every attack aimed at the player shows.
        for (kind, z) in [(2, 3.), (8, 3.), (4, 3.), (9, 3.), (3, 3.), (7, 3.), (11, 7.)] {
            let g = setup(kind, 0.5, z);
            assert!(g.run.enemies[0].ai.warning > 0., "{kind} began its warning");
            let threats = g.unseen_threats(half_view);
            assert_eq!(threats.len(), 1, "kind {kind}");
            let relative = threats[0].bearing - g.run.yaw;
            assert!(relative.cos() < -0.9, "kind {kind} points behind");
            assert!(threats[0].urgency < 0.1, "kind {kind} just began");
        }
        // Summons don't target the player.
        let g = setup(10, 0.5, 3.);
        assert!(g.run.enemies[0].ai.warning > 0.);
        assert!(g.unseen_threats(half_view).is_empty());
        // In view, the attack's own warning is visible.
        let g = setup(2, 0.5, -3.);
        assert!(g.run.enemies[0].ai.warning > 0.);
        assert!(g.unseen_threats(half_view).is_empty());
        // A burst whose circle the player has left is no threat.
        let mut g = setup(7, 0.5, 3.);
        g.run.pos = Vec3::new(0., 1.65, -6.);
        assert!(g.unseen_threats(half_view).is_empty());
        // Bearings in all four quadrants, relative to the view.
        for (yaw, x, z, expected) in [
            (0., 0., 3., PI),
            (0., 3., 0., FRAC_PI_2),
            (0., -3., 0., -FRAC_PI_2),
            (FRAC_PI_2, 0., 3., FRAC_PI_2),
            (FRAC_PI_2, -3., 0., PI),
        ] {
            let mut g = setup(2, x, z);
            g.run.yaw = yaw;
            let threats = g.unseen_threats(half_view);
            assert_eq!(threats.len(), 1, "({x}, {z}) at yaw {yaw}");
            let r = threats[0].bearing - yaw;
            assert!(
                (r.sin() - expected.sin()).abs() < 0.2 && (r.cos() - expected.cos()).abs() < 0.2,
                "({x}, {z}) at yaw {yaw}: {r}"
            );
        }
        // Urgency rises through the warning; a dive in flight is most urgent.
        let mut g = setup(4, 0.5, 3.);
        g.run.enemies[0].ai.warning = crate::encounters::warning_time(4) * 0.25;
        assert!((g.unseen_threats(half_view)[0].urgency - 0.75).abs() < 0.01);
        g.run.enemies[0].ai.warning = 0.;
        g.run.enemies[0].ai.charging = 0.5;
        assert_eq!(g.unseen_threats(half_view)[0].urgency, 1.);
        // At most four, the most urgent kept.
        let mut g = setup(2, 0.5, 3.);
        let template = g.run.enemies[0].clone();
        g.run.enemies = (0..6)
            .map(|i| {
                let mut e = template.clone();
                e.pos = Vec3::new(i as f32 - 2.5, 0., 4.);
                e.ai.warning = 0.1 + i as f32 * 0.1;
                e
            })
            .collect();
        let threats = g.unseen_threats(half_view);
        assert_eq!(threats.len(), 4);
        assert!(threats.windows(2).all(|w| w[0].urgency >= w[1].urgency));
        let least = 1. - 0.4 / crate::encounters::warning_time(2);
        assert!((threats[3].urgency - least).abs() < 0.01);
    }
    #[test]
    fn all_33_weapons_deal_damage_and_roundtrip() {
        assert_eq!(WeaponKind::ALL.len(), 33);
        for kind in WeaponKind::ALL {
            let mut g = Game::new(false);
            g.new_run();
            g.run.pos = Vec3::new(0., 1.65, 0.);
            g.run.pitch = (-0.45f32).atan2(2.);
            g.run.weapon.kind = kind;
            g.run.ammo = g.run.weapon.capacity();
            g.run.enemies = vec![target(0., -2.)];
            g.fire(false);
            for _ in 0..90 {
                g.update(1. / 120.);
            }
            assert!(
                g.run.enemies.is_empty() || g.run.enemies[0].hp < 500.,
                "{:?} failed to damage",
                kind
            );
            let data = serde_json::to_vec(&g.run.weapon).unwrap();
            let card: Card = serde_json::from_slice(&data).unwrap();
            assert_eq!(card.kind, kind);
            if kind.melee() {
                assert_eq!(g.run.ammo, 0);
                assert_eq!(g.reload, 0.);
            }
        }
    }
    #[test]
    fn traits_change_combat_and_melee_respects_reach() {
        for kind in [
            WeaponKind::Longrifle,
            WeaponKind::StormWand,
            WeaponKind::GraveRevolver,
            WeaponKind::Dragonbreath,
            WeaponKind::Frostbite,
            WeaponKind::Needler,
            WeaponKind::SoulLantern,
        ] {
            let mut g = Game::new(false);
            g.new_run();
            g.run.pos = Vec3::new(0., 1.2, 0.);
            g.run.hp = 50.;
            g.run.weapon.kind = kind;
            g.run.ammo = g.run.weapon.capacity();
            g.run.enemies = vec![target(0., -3.), target(0., -5.)];
            g.fire(false);
            for _ in 0..24 {
                g.update(1. / 120.);
            }
            match kind.spec().effect {
                Effect::Pierce | Effect::Chain => assert!(g.run.enemies[1].hp < 500.),
                Effect::Burn => assert!(g.run.enemies[0].burn > 0.),
                Effect::Frost => assert!(g.run.enemies[0].slow > 0.),
                Effect::Poison => assert!(g.run.enemies[0].poison > 0.),
                Effect::Drain => assert!(g.run.hp > 50.),
                _ => assert!(g.run.enemies[0].pos.z < -3.),
            }
        }
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 0.);
        g.run.weapon.kind = WeaponKind::TwinDaggers;
        g.run.enemies = vec![target(0., -6.)];
        g.fire(false);
        for _ in 0..40 {
            g.update(1. / 120.);
        }
        assert_eq!(g.run.enemies[0].hp, 500.);
    }
    #[test]
    fn bursts_spend_each_round_and_melee_waits_for_contact() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 0.);
        g.run.enemies = vec![target(0., -20.)];
        g.run.weapon.kind = WeaponKind::Carbine;
        g.run.ammo = 3;
        g.fire(false);
        assert_eq!(g.run.ammo, 2);
        g.reload_weapon();
        assert_eq!(g.reload, 0.);
        for _ in 0..4 {
            g.update(0.01);
        }
        assert_eq!(g.run.ammo, 2);
        for _ in 0..4 {
            g.update(0.01);
        }
        assert_eq!(g.run.ammo, 1);
        for _ in 0..8 {
            g.update(0.01);
        }
        assert_eq!(g.run.ammo, 0);
        assert_eq!(g.burst_remaining, 0);

        g.reset_effects();
        g.run.weapon.kind = WeaponKind::Cleaver;
        g.run.enemies = vec![target(0., -2.)];
        g.fire(false);
        assert_eq!(g.run.enemies[0].hp, 500.);
        g.update(0.01);
        assert_eq!(g.run.enemies[0].hp, 500.);
        for _ in 0..40 {
            g.update(0.01);
        }
        assert!(g.run.enemies[0].hp < 500.);
        assert!(!g.melee_pending);
    }
    #[test]
    fn loot_covers_the_armory_and_practice_restores_the_run() {
        let mut g = Game::new(false);
        let mut found = [false; 33];
        for _ in 0..3000 {
            found[g.roll_card().kind as usize] = true;
        }
        assert!(found.into_iter().all(|x| x));
        g.run.gold = 173;
        g.mode = Mode::Collection;
        g.return_mode = Mode::Shop;
        let before = serde_json::to_vec(&g.run).unwrap();
        g.practice(Card {
            kind: WeaponKind::Scythe,
            ..Card::starter()
        });
        assert!(g.practice_backup.is_some());
        g.run.gold = 0;
        g.back();
        assert_eq!(g.mode, Mode::Collection);
        assert_eq!(g.return_mode, Mode::Shop);
        assert_eq!(serde_json::to_vec(&g.run).unwrap(), before);
    }
    #[test]
    fn reload_cues_are_once_only_and_ammo_waits_for_completion() {
        for step in [1. / 30., 1. / 144.] {
            for kind in [WeaponKind::Pistol, WeaponKind::Double, WeaponKind::Repeater] {
                let mut g = Game::new(false);
                g.new_run();
                g.run.weapon.kind = kind;
                g.run.weapon.paths[1] = 3;
                g.run.ammo = 0;
                g.reload_weapon();
                g.sound_events.clear();
                while g.reload > 0. {
                    assert_eq!(g.run.ammo, 0);
                    g.fire(false);
                    assert_eq!(g.run.ammo, 0);
                    g.update(step);
                }
                assert_eq!(g.run.ammo, g.run.weapon.capacity());
                let cues: Vec<_> = g
                    .sound_events
                    .iter()
                    .copied()
                    .filter(|e| *e != "hurt")
                    .collect();
                let expected: Vec<_> = crate::motion::reload_cues(kind)
                    .iter()
                    .map(|(_, e)| *e)
                    .collect();
                assert_eq!(cues, expected);
            }
        }
    }
    #[test]
    fn full_run_reaches_warden_and_victory_through_combat() {
        let mut g = Game::new(false);
        g.new_run();
        let mut saw_boss = false;
        for _ in 0..120000 {
            match g.mode {
                Mode::Arena => {
                    if let Some(e) = g.run.enemies.iter().min_by(|a, b| {
                        a.pos
                            .distance_squared(g.run.pos)
                            .total_cmp(&b.pos.distance_squared(g.run.pos))
                    }) {
                        saw_boss |= e.kind == 3;
                        let d = Pose::for_enemy(e, g.run.pos).anchor(
                            if e.anatomy.missing(Part::Head) {
                                Part::Torso
                            } else {
                                Part::Head
                            },
                        ) - g.run.pos;
                        g.input.forward = if d.length() > 5. { 0.65 } else { -0.25 };
                        g.run.yaw = d.x.atan2(-d.z);
                        g.run.pitch = (d.y / d.length()).asin();
                    }
                    g.input.fire = true;
                    g.input.right = 0.65;
                    if g.dash_cd <= 0.
                        && g.run.enemies.iter().any(|e| e.pos.distance(g.run.pos) < 3.)
                    {
                        g.dodge();
                    }
                    if g.run.chalice {
                        g.fire(true);
                    }
                    g.update(1. / 60.);
                }
                Mode::Shop => {
                    if g.run.wave == 1 {
                        g.buy_offer();
                    } else if !g.run.chalice && g.spend(65) {
                        g.run.chalice = true;
                    }
                    if g.run.gold >= 40 && g.spend(40) {
                        g.run.armor += 30.;
                    }
                    g.next_wave();
                }
                Mode::Victory => {
                    assert!(saw_boss);
                    assert_eq!(g.run.wave, crate::survival::DESCENTS);
                    assert!(g.run.kills >= 558);
                    assert!(g.run.survival.level > 8);
                    return;
                }
                Mode::LevelUp => {
                    let slot = g
                        .run
                        .survival
                        .choices
                        .iter()
                        .enumerate()
                        .min_by_key(|(_, p)| match p {
                            5 => 0,
                            0 => 1,
                            6 => 2,
                            7 => 3,
                            3 => 4,
                            4 => 5,
                            1 => 6,
                            _ => 7,
                        })
                        .unwrap()
                        .0;
                    g.choose_power(slot);
                }
                Mode::Dead => panic!(
                    "Run died on wave {} after {} kills",
                    g.run.wave, g.run.kills
                ),
                _ => panic!("Unexpected state"),
            }
        }
        panic!("Full run did not finish");
    }
    #[test]
    fn unaffordable_pack_does_not_mutate() {
        let mut g = Game::new(false);
        g.mode = Mode::Shop;
        g.open_pack();
        assert_eq!(g.run.gold, 0);
        assert_eq!(g.run.pack_buys, 0);
        assert_eq!(g.mode, Mode::Shop);
    }
    #[test]
    fn pack_cost_scales_and_resets() {
        let mut g = Game::new(false);
        g.run.gold = 100;
        g.open_pack();
        assert_eq!(g.run.gold, 70);
        g.select_pack(0);
        g.open_pack();
        assert_eq!(g.run.gold, 25);
        g.select_pack(0);
        g.complete_wave();
        assert_eq!(g.run.pack_buys, 0);
    }
    #[test]
    fn progression_survives_rounds_and_serialization() {
        let mut g = Game::new(false);
        g.run.gold = 500;
        g.upgrade(0);
        let damage = g.run.weapon.damage();
        g.complete_wave();
        g.next_wave();
        assert_eq!(g.run.weapon.damage(), damage);
        let data = serde_json::to_string(&g.run).unwrap();
        let restored: Run = serde_json::from_str(&data).unwrap();
        assert_eq!(restored.weapon.paths, [1, 0, 0]);
    }
    #[test]
    fn shots_damage_and_kill() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.enemies = vec![Enemy {
            pos: Vec3::new(0., 0., 3.),
            hp: 20.,
            max_hp: 20.,
            kind: 0,
            attack: 0.,
            phase: 0.,
            hit: 0.,
            burn: 0.,
            poison: 0.,
            slow: 0.,
            anatomy: Anatomy::default(),
            ai: crate::encounters::Brain::default(),
        }];
        g.run.pitch = ((1.2 - g.run.pos.y) / (g.run.pos - Vec3::new(0., 1.2, 3.)).length()).asin();
        g.fire(false);
        assert!(g.run.enemies.is_empty());
        assert_eq!(g.run.kills, 1);
        assert_eq!(g.run.ammo, 7);
        assert!(!g.spawned_bodies.is_empty());
    }
    #[test]
    fn death_and_victory_are_distinct() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.hp = 0.;
        g.update(0.016);
        assert_eq!(g.mode, Mode::Dead);
        g.new_run();
        g.run.wave = crate::survival::DESCENTS;
        g.run.survival.remaining = 0;
        g.run.enemies.clear();
        g.update(0.016);
        assert_eq!(g.mode, Mode::Victory);
    }
    #[test]
    fn each_attack_family_records_its_cause_and_the_killing_blow() {
        let arena = || {
            let mut g = Game::new(false);
            g.new_run();
            g.mode = Mode::Arena;
            g.run.survival.remaining = 0;
            g.run.pos = Vec3::new(0., 1.65, 0.);
            g
        };
        let cause = |kind, attack| Some(Cause { kind, attack });
        // A melee strike from an adjacent Ribblade Skirmisher.
        let mut g = arena();
        let mut attacker = target(1.2, 0.);
        attacker.kind = 1;
        attacker.attack = 0.;
        g.run.enemies = vec![attacker];
        while g.run.stats.last_hit.is_none() && g.run.time < 3. {
            g.update(0.01);
        }
        assert_eq!(g.run.stats.last_hit, cause(1, Attack::Strike));
        // An Ash Cantor's bolt.
        let mut g = arena();
        g.hazards.push(crate::encounters::Hazard {
            pos: Vec3::new(0., 1.5, -2.),
            vel: Vec3::Z * 6.,
            life: 5.,
            damage: 8.,
            color: [1., 0., 1.],
            from: 8,
        });
        for _ in 0..60 {
            g.tick_enemies(0.01);
        }
        assert_eq!(g.run.stats.last_hit, cause(8, Attack::Bolt));
        assert!((g.run.stats.damage_taken - 8.).abs() < 1e-4);
        // The Tithekeeper's slam and a Plague Vessel's burst land when their
        // warnings run out with the player inside the marked circle.
        for (kind, attack) in [(3, Attack::Slam), (7, Attack::Burst)] {
            let mut g = arena();
            let mut e = Enemy::spawn(kind, Vec3::new(2.5, 0., 0.), 1, 0.);
            e.ai.warning = 0.005;
            e.ai.target = g.run.pos * Vec3::new(1., 0., 1.);
            g.run.enemies = vec![e];
            g.tick_enemies(0.01);
            assert_eq!(g.run.stats.last_hit, cause(kind, attack), "kind {kind}");
        }
        // The blow that empties vitality is the one the death screen names,
        // and damage taken stops at the vitality that was left.
        let mut g = arena();
        g.run.hp = 3.;
        g.hazards.push(crate::encounters::Hazard {
            pos: Vec3::new(0., 1.5, -0.3),
            vel: Vec3::Z * 6.,
            life: 5.,
            damage: 8.,
            color: [1., 0.5, 0.],
            from: 2,
        });
        g.update(0.05);
        assert_eq!(g.mode, Mode::Dead);
        assert_eq!(g.run.stats.last_hit, cause(2, Attack::Bolt));
        assert!((g.run.stats.damage_taken - 3.).abs() < 1e-4);
        assert_eq!(
            g.run.stats.last_hit.unwrap().describe(),
            "A CINDER SKULL'S BOLT"
        );
        assert_eq!(
            Cause { kind: 3, attack: Attack::Slam }.describe(),
            "THE TITHEKEEPER'S SLAM"
        );
        assert_eq!(
            Cause { kind: 8, attack: Attack::Bolt }.describe(),
            "AN ASH CANTOR'S BOLT"
        );
    }
    #[test]
    fn wounds_are_shared_by_raw_damage_and_the_top_source_is_reported() {
        let mut stats = RunStats::default();
        assert_eq!(stats.top_source(), None);
        let hit = |kind, amount| {
            (Vec3::ZERO, Cause { kind, attack: Attack::Strike }, amount)
        };
        // Armor absorbed part of a 30-point update; 12 vitality was lost.
        stats.record_wounds(&[hit(5, 10.), hit(3, 20.)], 12.);
        assert_eq!(stats.last_hit.map(|c| c.kind), Some(3));
        assert!((stats.taken_from[3] - 8.).abs() < 1e-4 && (stats.taken_from[5] - 4.).abs() < 1e-4);
        stats.record_wounds(&[hit(5, 9.)], 9.);
        assert_eq!(stats.last_hit.map(|c| c.kind), Some(5));
        let (kind, share) = stats.top_source().unwrap();
        assert_eq!(kind, 5);
        assert!((share - 13. / 21.).abs() < 1e-4);
        assert!((stats.damage_taken - 21.).abs() < 1e-4);
    }
    #[test]
    fn hits_and_kills_count_toward_the_run_summary() {
        let mut g = Game::new(false);
        g.new_run();
        g.run.pos = Vec3::new(0., 1.65, 4.);
        let mut front = target(0., 0.);
        front.hp = 100.;
        front.max_hp = 100.;
        g.run.enemies = vec![front.clone()];
        g.ray_attack(Vec3::new(0., 1.2, 4.), -Vec3::Z, WeaponKind::Pistol, 20.);
        assert!((g.run.stats.damage_dealt - 20.).abs() < 1e-4);
        assert_eq!(g.run.stats.headshots, 0);
        // A lethal headshot counts only the health the enemy had left.
        g.projectiles = vec![Projectile {
            pos: Vec3::new(0., 1.74, 3.),
            vel: -Vec3::Z * 100.,
            life: 2.,
            damage: 500.,
            kind: WeaponKind::Crossbow,
        }];
        g.update_projectiles(0.05);
        assert_eq!(g.run.stats.headshots, 1);
        assert!((g.run.stats.damage_dealt - 100.).abs() < 1e-4);
        assert_eq!(g.run.kills, 1);
        // The statistics persist with the run; older saves load with zeros.
        let json = serde_json::to_value(&g.run).unwrap();
        let back: Run = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back.stats, g.run.stats);
        let mut legacy = json;
        legacy.as_object_mut().unwrap().remove("stats");
        let old: Run = serde_json::from_value(legacy).unwrap();
        assert_eq!(old.stats, RunStats::default());
        // Practice uses a copy of the run and leaves the real one untouched.
        let before = g.run.stats.clone();
        g.device = Device::Controller;
        g.practice(Card::starter());
        assert!(g.notice.contains("START returns to the armory"), "{}", g.notice);
        let practice_start = g.run.stats.damage_dealt;
        g.run.pos = Vec3::new(0., 1.65, 4.);
        g.run.enemies = vec![front];
        g.ray_attack(Vec3::new(0., 1.2, 4.), -Vec3::Z, WeaponKind::Pistol, 20.);
        assert!(g.run.stats.damage_dealt > practice_start);
        g.back();
        assert!(g.practice_backup.is_none());
        assert_eq!(g.run.stats, before);
    }
}
