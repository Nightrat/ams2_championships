use serde::Serialize;

/// Real-time participant data extracted from AMS2 shared memory.
#[derive(Serialize, Clone)]
pub struct ParticipantData {
    pub name: String,
    pub car_name: String,
    pub car_class: String,
    pub is_active: bool,
    /// True when this participant is the viewed/human player (mViewedParticipantIndex).
    pub is_player: bool,
    pub race_position: u32,
    pub laps_completed: u32,
    pub current_lap: u32,
    /// Distance into the current lap in metres (mCurrentLapDistance).
    pub current_lap_distance: f32,
    /// Current lap's completed sector times — -1 means not yet set this lap.
    pub cur_s1: f32,
    pub cur_s2: f32,
    pub cur_s3: f32,
    /// Personal best sector times across the session — -1 means not yet set.
    pub best_s1: f32,
    pub best_s2: f32,
    pub best_s3: f32,
    pub fastest_lap_time: f32,
    pub last_lap_time: f32,
    /// World X position in metres.
    pub world_pos_x: f32,
    /// World Z position in metres (horizontal plane with X).
    pub world_pos_z: f32,
    /// Race interval to the car directly ahead, in seconds. -1.0 = leader, 0.0 = not applicable.
    pub interval_gap_secs: f32,
    /// Whole laps behind the car directly ahead (0 = same lap).
    pub interval_gap_laps: u32,
    /// True when the participant is in the pit lane or garage (mCurrentSector == -1).
    pub in_pits: bool,
}

/// Player car telemetry from the AMS2 shared memory (player's car only).
/// Wheel order for all arrays: FL=0, FR=1, RL=2, RR=3.
#[derive(Serialize, Clone)]
pub struct PlayerTelemetry {
    /// Left-edge tyre temperature, °C.
    pub tyre_temp_left: [f32; 4],
    /// Centre-tread tyre temperature, °C.
    pub tyre_temp_center: [f32; 4],
    /// Right-edge tyre temperature, °C.
    pub tyre_temp_right: [f32; 4],
    /// Tyre wear 0–1 (0 = new, 1 = fully worn).
    pub tyre_wear: [f32; 4],
    /// Tyre state bits (mTyreFlags): 1 = attached, 2 = inflated, 4 = on the ground.
    pub tyre_flags: [u32; 4],
    /// Terrain material under each wheel (mTerrain) — see `TERRAIN_NAMES` in the UI.
    pub terrain: [u32; 4],
    /// Tyre position, local-space Y (mTyreY).
    pub tyre_y: [f32; 4],
    /// Wheel rotation in revolutions per second (mTyreRPS). Negative under reverse.
    pub tyre_rps: [f32; 4],
    /// Tyre surface temperature, °C. 0 means unset, per the header.
    pub tyre_temp: [f32; 4],
    /// Height of the tyre above the ground, local-space Y (mTyreHeightAboveGround).
    pub tyre_height_above_ground: [f32; 4],
    /// Tread temperature, °C. Converted from the Kelvin the header specifies.
    pub tyre_tread_temp: [f32; 4],
    /// Layer temperature, °C (from Kelvin).
    pub tyre_layer_temp: [f32; 4],
    /// Carcass temperature, °C (from Kelvin).
    pub tyre_carcass_temp: [f32; 4],
    /// Rim temperature, °C (from Kelvin).
    pub tyre_rim_temp: [f32; 4],
    /// Internal air temperature, °C (from Kelvin).
    pub tyre_internal_air_temp: [f32; 4],
    /// Marked OBSOLETE in the PCars2 header — "kept for backward compatibility
    /// only". Read anyway so the panel can show they are dead rather than leaving
    /// it an open question every time someone wants slip or grip data.
    pub tyre_slip_speed: [f32; 4],
    /// Obsolete, see `tyre_slip_speed`.
    pub tyre_grip: [f32; 4],
    /// Obsolete, see `tyre_slip_speed`.
    pub tyre_lateral_stiffness: [f32; 4],
    /// Suspension velocity, m/s (mSuspensionVelocity).
    pub suspension_velocity: [f32; 4],
    /// Wheel position, local-space Y (mWheelLocalPositionY).
    pub wheel_local_position_y: [f32; 4],
    /// Tyre air pressure, PSI.
    pub tyre_pressure: [f32; 4],
    /// Brake disc temperature, °C.
    pub brake_temp: [f32; 4],
    /// Suspension travel, metres.
    pub suspension_travel: [f32; 4],
    /// Ride height per corner, cm.
    pub ride_height: [f32; 4],
    /// Filtered throttle 0–1.
    pub throttle: f32,
    /// Filtered brake 0–1.
    pub brake_input: f32,
    /// Filtered steering −1…+1.
    pub steering: f32,
    /// Speed m/s.
    pub speed: f32,
    /// Engine RPM.
    pub rpm: f32,
    /// Current gear (negative = reverse, 0 = neutral).
    pub gear: i32,
    /// Tyre compound name per wheel (FL, FR, RL, RR).
    pub tyre_compound: [String; 4],
    /// Current fuel level in litres (mFuelLevel).
    pub fuel_level: f32,
    /// Fuel tank capacity in litres (mFuelCapacity).
    pub fuel_capacity: f32,
    /// mCrashState: 0=none, 1=off-track, 2=hit scenery, 3=spinning, 4=rolling.
    pub crash_state: u32,
    /// Bodywork damage 0–1 (0 = intact). The one that costs lap time.
    pub aero_damage: f32,
    /// Engine damage 0–1.
    pub engine_damage: f32,
    /// Brake damage per corner, 0–1.
    pub brake_damage: [f32; 4],
    /// Suspension damage per corner, 0–1.
    pub suspension_damage: [f32; 4],
    /// Participant index of the last car hit; -1 when nothing has been hit.
    pub last_collision_index: i32,
    /// Force of that last contact. Non-zero without any damage means a clean tap.
    pub last_collision_magnitude: f32,
}

/// Snapshot of the current AMS2 session state.
#[derive(Serialize, Clone)]
pub struct LiveSessionData {
    pub connected: bool,
    pub game_state: u32,
    pub session_state: u32,
    pub race_state: u32,
    pub num_participants: i32,
    pub track_location: String,
    pub track_variation: String,
    /// Total track length in metres (mTrackLength), used for gap calculation.
    pub track_length: f32,
    /// Total laps in the event (mLapsInEvent). 0 = time-based / unknown.
    pub laps_in_event: u32,
    /// Player's car name (mCarName).
    pub car_name: String,
    /// Player's car class name (mCarClassName).
    pub car_class: String,
    pub participants: Vec<ParticipantData>,
    pub player_telemetry: PlayerTelemetry,
    /// mHighestFlagColour: 0=none,1=green,2=blue,3=white slow,4=white final lap,5=red,6=yellow,7=double yellow,8=black&white,9=meatball,10=black,11=chequered
    pub race_flag_colour: u32,
    /// mHighestFlagReason: 0=none,1=solo crash,2=stopped,3=oil,4=gravel,5=recovery,6=SC deployed,7=SC returning
    pub race_flag_reason: u32,
    /// mPitMode: 0=none, 1=driving into pits, 2=in pit, 3=driving out of pits,
    /// 4=in garage, 5=driving out of garage. Anything but 0 means off the track —
    /// and unlike `ParticipantData::in_pits` it catches ESC → "Return to pits",
    /// which teleports the car to the garage without it ever entering a pit lane.
    pub pit_mode: u32,
}

fn disconnected() -> LiveSessionData {
    LiveSessionData {
        connected: false,
        game_state: 0,
        session_state: 0,
        race_state: 0,
        num_participants: 0,
        track_location: String::new(),
        track_variation: String::new(),
        track_length: 0.0,
        laps_in_event: 0,
        car_name: String::new(),
        car_class: String::new(),
        participants: vec![],
        race_flag_colour: 0,
        race_flag_reason: 0,
        pit_mode: 0,
        player_telemetry: PlayerTelemetry {
            tyre_temp_left: [0.0; 4],
            tyre_temp_center: [0.0; 4],
            tyre_temp_right: [0.0; 4],
            tyre_wear: [0.0; 4],
            tyre_flags: [0; 4],
            terrain: [0; 4],
            tyre_y: [0.0; 4],
            tyre_rps: [0.0; 4],
            tyre_temp: [0.0; 4],
            tyre_height_above_ground: [0.0; 4],
            tyre_tread_temp: [0.0; 4],
            tyre_layer_temp: [0.0; 4],
            tyre_carcass_temp: [0.0; 4],
            tyre_rim_temp: [0.0; 4],
            tyre_internal_air_temp: [0.0; 4],
            tyre_slip_speed: [0.0; 4],
            tyre_grip: [0.0; 4],
            tyre_lateral_stiffness: [0.0; 4],
            suspension_velocity: [0.0; 4],
            wheel_local_position_y: [0.0; 4],
            tyre_pressure: [0.0; 4],
            brake_temp: [0.0; 4],
            suspension_travel: [0.0; 4],
            ride_height: [0.0; 4],
            throttle: 0.0,
            brake_input: 0.0,
            steering: 0.0,
            speed: 0.0,
            rpm: 0.0,
            gear: 0,
            tyre_compound: [String::new(), String::new(), String::new(), String::new()],
            fuel_level: 0.0,
            fuel_capacity: 0.0,
            crash_state: 0,
            aero_damage: 0.0,
            engine_damage: 0.0,
            brake_damage: [0.0; 4],
            suspension_damage: [0.0; 4],
            last_collision_index: -1,
            last_collision_magnitude: 0.0,
        },
    }
}

// ── Windows shared memory reader ─────────────────────────────────────────────

#[cfg(windows)]
pub fn read_live_session() -> LiveSessionData {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Memory::{
        MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ,
    };

    // pCars2 / AMS2 shared memory name
    let smname: Vec<u16> = OsStr::new("$pcars2$")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // ── SharedMemory top-level offsets ────────────────────────────────────────
    // Based on CREST2-AMS2 SharedMemory.h (viper4gh/CREST2-AMS2)
    //
    // unsigned int mVersion;                  //     0  (4)
    // unsigned int mBuildVersionNumber;       //     4  (4)
    // unsigned int mGameState;                //     8  (4)
    // unsigned int mSessionState;             //    12  (4)
    // unsigned int mRaceState;                //    16  (4)
    // int mViewedParticipantIndex;            //    20  (4)
    // int mNumParticipants;                   //    24  (4)
    // ParticipantInfo mParticipantInfo[64];   //    28  (100 bytes each × 64 = 6400)
    // ... (many single-value fields)          //  6428 onwards
    // char mTrackLocation[64];                //  6576  (64)
    // ... (many more fields)                  //
    // float mHandBrake;                       //  7404  (4)
    // float mCurrentSector1Times[64];         //  7408  (256)
    // float mCurrentSector2Times[64];         //  7664  (256)
    // float mCurrentSector3Times[64];         //  7920  (256)
    // float mFastestSector1Times[64];         //  8176  (256)
    // float mFastestSector2Times[64];         //  8432  (256)
    // float mFastestSector3Times[64];         //  8688  (256)
    // float mFastestLapTimes[64];             //  8944  (256)
    // float mLastLapTimes[64];                //  9200  (256)
    const OFF_GAME_STATE: usize = 8;
    const OFF_SESSION_STATE: usize = 12;
    const OFF_RACE_STATE: usize = 16;
    const OFF_VIEWED_PARTICIPANT: usize = 20;
    const OFF_NUM_PARTICIPANTS: usize = 24;
    const OFF_PARTICIPANTS: usize = 28;
    const OFF_CAR_NAME: usize = 6444; // char mCarName[64]
    const OFF_CAR_CLASS: usize = 6508; // char mCarClassName[64]
    const OFF_LAPS_IN_EVENT: usize = 6572; // unsigned int mLapsInEvent
    const OFF_TRACK_LOCATION: usize = 6576; // char mTrackLocation[64]
    const OFF_TRACK_VARIATION: usize = 6640; // char mTrackVariation[64]
    const OFF_TRACK_LENGTH: usize = 6704;
    // Sector time arrays (all float[64], indexed by participant slot)
    // float mCurrentSector1Times[64]  //  7408  (256)
    // float mCurrentSector2Times[64]  //  7664  (256)
    // float mCurrentSector3Times[64]  //  7920  (256)
    // float mFastestSector1Times[64]  //  8176  (256)
    // float mFastestSector2Times[64]  //  8432  (256)
    // float mFastestSector3Times[64]  //  8688  (256)
    // float mFastestLapTimes[64]      //  8944  (256)
    // float mLastLapTimes[64]         //  9200  (256)
    const OFF_CAR_NAMES: usize = 11056; // char mCarNames[64][64]
    const OFF_CAR_CLASS_NAMES: usize = 15152; // char mCarClassNames[64][64]
                                              // ── Player car telemetry ──────────────────────────────────────────────────
    const OFF_SPEED: usize = 6848; // float mSpeed (m/s)
    const OFF_RPM: usize = 6852; // float mRpm
    const OFF_BRAKE_INPUT: usize = 6860; // float mBrake (filtered)
    const OFF_THROTTLE: usize = 6864; // float mThrottle (filtered)
    const OFF_STEERING: usize = 6872; // float mSteering (filtered)
    const OFF_GEAR: usize = 6876; // int mGear
    // ── Wheels / tyres, one float[4] or u32[4] per row, FL FR RL RR ───────────
    const OFF_TYRE_FLAGS: usize = 6992; // unsigned int mTyreFlags[4]
    const OFF_TERRAIN: usize = 7008; // unsigned int mTerrain[4]
    const OFF_TYRE_Y: usize = 7024; // float mTyreY[4]
    const OFF_TYRE_RPS: usize = 7040; // float mTyreRPS[4]
    const OFF_TYRE_SLIP_SPEED: usize = 7056; // float mTyreSlipSpeed[4]  (OBSOLETE)
    const OFF_TYRE_TEMP: usize = 7072; // float mTyreTemp[4] (°C, 0 = unset)
    const OFF_TYRE_GRIP: usize = 7088; // float mTyreGrip[4]  (OBSOLETE)
    const OFF_TYRE_HEIGHT_ABOVE_GROUND: usize = 7104; // float mTyreHeightAboveGround[4]
    const OFF_TYRE_LATERAL_STIFFNESS: usize = 7120; // float mTyreLateralStiffness[4] (OBSOLETE)
    const OFF_TYRE_WEAR: usize = 7136; // float mTyreWear[4]
    const OFF_BRAKE_DAMAGE: usize = 7152; // float mBrakeDamage[4]
    const OFF_SUSPENSION_DAMAGE: usize = 7168; // float mSuspensionDamage[4]
    const OFF_BRAKE_TEMP: usize = 7184; // float mBrakeTempCelsius[4]
    // The five carcass-side temperatures are KELVIN in the header, unlike every
    // other temperature in the struct. They are converted on read — see kelvin4.
    const OFF_TYRE_TREAD_TEMP: usize = 7200; // float mTyreTreadTemp[4] (K)
    const OFF_TYRE_LAYER_TEMP: usize = 7216; // float mTyreLayerTemp[4] (K)
    const OFF_TYRE_CARCASS_TEMP: usize = 7232; // float mTyreCarcassTemp[4] (K)
    const OFF_TYRE_RIM_TEMP: usize = 7248; // float mTyreRimTemp[4] (K)
    const OFF_TYRE_INTERNAL_AIR_TEMP: usize = 7264; // float mTyreInternalAirTemp[4] (K)
    const OFF_CRASH_STATE: usize = 7280; // unsigned int mCrashState
    const OFF_AERO_DAMAGE: usize = 7284; // float mAeroDamage
    const OFF_ENGINE_DAMAGE: usize = 7288; // float mEngineDamage
    // mAntiLockActive (bool) at 6888 pads to 4, so the collision pair starts at 6892.
    const OFF_LAST_COLLISION_INDEX: usize = 6892; // int mLastOpponentCollisionIndex (-1 = none)
    const OFF_LAST_COLLISION_MAG: usize = 6896; // float mLastOpponentCollisionMagnitude
    const OFF_WHEEL_LOCAL_POSITION_Y: usize = 7324; // float mWheelLocalPositionY[4]
    const OFF_SUSPENSION_TRAVEL: usize = 7340; // float mSuspensionTravel[4] (metres)
    const OFF_SUSPENSION_VELOCITY: usize = 7356; // float mSuspensionVelocity[4] (m/s)
    const OFF_TYRE_PRESSURE: usize = 7372; // float mAirPressure[4] (PSI)
                                           // AMS2-specific additions (not in original PC2 header):
    const OFF_TYRE_COMPOUND: usize = 19388; // char mTyreCompound[4][40]
    const OFF_TYRE_TEMP_LEFT: usize = 20584; // float mTyreTempLeft[4] (°C)
    const OFF_TYRE_TEMP_CENTER: usize = 20600; // float mTyreTempCenter[4] (°C)
    const OFF_TYRE_TEMP_RIGHT: usize = 20616; // float mTyreTempRight[4] (°C)
    const OFF_RIDE_HEIGHT: usize = 20636; // float mRideHeight[4] (cm)
    const OFF_CUR_S1: usize = 7408;
    const OFF_CUR_S2: usize = 7664;
    const OFF_CUR_S3: usize = 7920;
    const OFF_BEST_S1: usize = 8176;
    const OFF_BEST_S2: usize = 8432;
    const OFF_BEST_S3: usize = 8688;
    const OFF_FASTEST_LAP_TIMES: usize = 8944;
    const OFF_LAST_LAP_TIMES: usize = 9200;
    // ── Flags, pit info and car state ─────────────────────────────────────────
    // These do NOT follow mLastLapTimes. They sit between the single-value sector
    // times and mSpeed, which anchors them: counting back field by field from
    // mSpeed (6848) lands mHighestFlagColour on 6800, and every value in between
    // reads as its own unit — oil ~98 °C, water ~68 °C, a 250 litre tank.
    const OFF_HIGHEST_FLAG_COLOUR: usize = 6800; // unsigned int mHighestFlagColour
    const OFF_HIGHEST_FLAG_REASON: usize = 6804; // unsigned int mHighestFlagReason
    const OFF_PIT_MODE: usize = 6808; // unsigned int mPitMode
    const OFF_FUEL_LEVEL: usize = 6840; // float mFuelLevel — a FRACTION, 0..1
    const OFF_FUEL_CAPACITY: usize = 6844; // float mFuelCapacity (litres)

    // ── ParticipantInfo layout (100 bytes each) ───────────────────────────────
    // bool mIsActive;              // +  0  (1)
    // char mName[64];              // +  1  (64)   ← no padding between bool and char
    // [3 bytes padding]            // + 65
    // float mWorldPosition[3];     // + 68  (12)
    // float mCurrentLapDistance;   // + 80  (4)
    // unsigned int mRacePosition;  // + 84  (4)
    // unsigned int mLapsCompleted; // + 88  (4)
    // unsigned int mCurrentLap;    // + 92  (4)
    // int mCurrentSector;          // + 96  (4)
    // total: 100 bytes
    //
    // NOTE: lap times are NOT in ParticipantInfo — they live in top-level arrays
    //       indexed by participant index (mFastestLapTimes[i], mLastLapTimes[i]).
    const STRIDE: usize = 100;
    const P_IS_ACTIVE: usize = 0;
    const P_NAME: usize = 1;
    // mWorldPosition[3] floats at +68 (+72 = Y skipped, +76 = Z)
    const P_WORLD_POS: usize = 68;
    const P_CUR_LAP_DIST: usize = 80;
    const P_RACE_POS: usize = 84;
    const P_LAPS_DONE: usize = 88;
    const P_CUR_LAP: usize = 92;
    const P_CURRENT_SECTOR: usize = 96; // int, -1 = pits/garage

    unsafe fn rf32x4(b: *const u8, off: usize) -> [f32; 4] {
        [
            rf32(b, off),
            rf32(b, off + 4),
            rf32(b, off + 8),
            rf32(b, off + 12),
        ]
    }
    unsafe fn ru32x4(b: *const u8, off: usize) -> [u32; 4] {
        [
            ru32(b, off),
            ru32(b, off + 4),
            ru32(b, off + 8),
            ru32(b, off + 12),
        ]
    }
    /// Kelvin to Celsius, holding "unset" at zero. The header marks 0.0 as unset
    /// for these temperatures, and a straight subtraction would report an unset
    /// wheel as −273 °C — a number that looks like a bad offset rather than no
    /// data, which is exactly the confusion this panel exists to avoid.
    unsafe fn kelvin4(b: *const u8, off: usize) -> [f32; 4] {
        rf32x4(b, off).map(|k| if k <= 0.0 { 0.0 } else { k - 273.15 })
    }
    unsafe fn ru8(b: *const u8, off: usize) -> u8 {
        *b.add(off)
    }
    unsafe fn ru32(b: *const u8, off: usize) -> u32 {
        (b.add(off) as *const u32).read_unaligned()
    }
    unsafe fn ri32(b: *const u8, off: usize) -> i32 {
        (b.add(off) as *const i32).read_unaligned()
    }
    unsafe fn rf32(b: *const u8, off: usize) -> f32 {
        (b.add(off) as *const f32).read_unaligned()
    }
    unsafe fn rstr(b: *const u8, off: usize, max: usize) -> String {
        let s = std::slice::from_raw_parts(b.add(off), max);
        let end = s.iter().position(|&c| c == 0).unwrap_or(max);
        String::from_utf8_lossy(&s[..end]).into_owned()
    }

    unsafe {
        let handle = OpenFileMappingW(FILE_MAP_READ, 0, smname.as_ptr());
        if handle == 0 {
            return disconnected();
        }

        let mapped = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0);
        if mapped.Value.is_null() {
            CloseHandle(handle);
            return disconnected();
        }
        let ptr = mapped.Value as *const u8;

        let player_telemetry = PlayerTelemetry {
            tyre_temp_left: rf32x4(ptr, OFF_TYRE_TEMP_LEFT),
            tyre_temp_center: rf32x4(ptr, OFF_TYRE_TEMP_CENTER),
            tyre_temp_right: rf32x4(ptr, OFF_TYRE_TEMP_RIGHT),
            tyre_wear: rf32x4(ptr, OFF_TYRE_WEAR),
            tyre_flags: ru32x4(ptr, OFF_TYRE_FLAGS),
            terrain: ru32x4(ptr, OFF_TERRAIN),
            tyre_y: rf32x4(ptr, OFF_TYRE_Y),
            tyre_rps: rf32x4(ptr, OFF_TYRE_RPS),
            tyre_temp: rf32x4(ptr, OFF_TYRE_TEMP),
            tyre_height_above_ground: rf32x4(ptr, OFF_TYRE_HEIGHT_ABOVE_GROUND),
            tyre_tread_temp: kelvin4(ptr, OFF_TYRE_TREAD_TEMP),
            tyre_layer_temp: kelvin4(ptr, OFF_TYRE_LAYER_TEMP),
            tyre_carcass_temp: kelvin4(ptr, OFF_TYRE_CARCASS_TEMP),
            tyre_rim_temp: kelvin4(ptr, OFF_TYRE_RIM_TEMP),
            tyre_internal_air_temp: kelvin4(ptr, OFF_TYRE_INTERNAL_AIR_TEMP),
            tyre_slip_speed: rf32x4(ptr, OFF_TYRE_SLIP_SPEED),
            tyre_grip: rf32x4(ptr, OFF_TYRE_GRIP),
            tyre_lateral_stiffness: rf32x4(ptr, OFF_TYRE_LATERAL_STIFFNESS),
            suspension_velocity: rf32x4(ptr, OFF_SUSPENSION_VELOCITY),
            wheel_local_position_y: rf32x4(ptr, OFF_WHEEL_LOCAL_POSITION_Y),
            tyre_pressure: rf32x4(ptr, OFF_TYRE_PRESSURE),
            brake_temp: rf32x4(ptr, OFF_BRAKE_TEMP),
            suspension_travel: rf32x4(ptr, OFF_SUSPENSION_TRAVEL),
            ride_height: rf32x4(ptr, OFF_RIDE_HEIGHT),
            throttle: rf32(ptr, OFF_THROTTLE),
            brake_input: rf32(ptr, OFF_BRAKE_INPUT),
            steering: rf32(ptr, OFF_STEERING),
            speed: rf32(ptr, OFF_SPEED),
            rpm: rf32(ptr, OFF_RPM),
            gear: ri32(ptr, OFF_GEAR),
            tyre_compound: [
                rstr(ptr, OFF_TYRE_COMPOUND, 40),
                rstr(ptr, OFF_TYRE_COMPOUND + 40, 40),
                rstr(ptr, OFF_TYRE_COMPOUND + 80, 40),
                rstr(ptr, OFF_TYRE_COMPOUND + 120, 40),
            ],
            // mFuelLevel is a fraction of the tank, not a volume. It is converted
            // here so `fuel_level / fuel_capacity` means what it reads as
            // everywhere downstream.
            fuel_level: rf32(ptr, OFF_FUEL_LEVEL) * rf32(ptr, OFF_FUEL_CAPACITY),
            fuel_capacity: rf32(ptr, OFF_FUEL_CAPACITY),
            crash_state: ru32(ptr, OFF_CRASH_STATE),
            aero_damage: rf32(ptr, OFF_AERO_DAMAGE),
            engine_damage: rf32(ptr, OFF_ENGINE_DAMAGE),
            brake_damage: rf32x4(ptr, OFF_BRAKE_DAMAGE),
            suspension_damage: rf32x4(ptr, OFF_SUSPENSION_DAMAGE),
            last_collision_index: ri32(ptr, OFF_LAST_COLLISION_INDEX),
            last_collision_magnitude: rf32(ptr, OFF_LAST_COLLISION_MAG),
        };
        let game_state = ru32(ptr, OFF_GAME_STATE);
        let session_state = ru32(ptr, OFF_SESSION_STATE);
        let race_state = ru32(ptr, OFF_RACE_STATE);
        let viewed_idx = ri32(ptr, OFF_VIEWED_PARTICIPANT);
        let num_participants = ri32(ptr, OFF_NUM_PARTICIPANTS).clamp(0, 64);
        let car_name = rstr(ptr, OFF_CAR_NAME, 64);
        let car_class = rstr(ptr, OFF_CAR_CLASS, 64);
        let laps_in_event = ru32(ptr, OFF_LAPS_IN_EVENT);
        let track_location = rstr(ptr, OFF_TRACK_LOCATION, 64);
        let track_variation = rstr(ptr, OFF_TRACK_VARIATION, 64);
        let track_length = rf32(ptr, OFF_TRACK_LENGTH);

        let mut participants = Vec::with_capacity(num_participants as usize);
        for i in 0..num_participants as usize {
            let base = OFF_PARTICIPANTS + i * STRIDE;
            let is_active = ru8(ptr, base + P_IS_ACTIVE) != 0;
            if !is_active {
                continue;
            }
            let name = rstr(ptr, base + P_NAME, 64);
            let car_name = rstr(ptr, OFF_CAR_NAMES + i * 64, 64);
            let car_class = rstr(ptr, OFF_CAR_CLASS_NAMES + i * 64, 64);

            // All timing arrays are at top-level, indexed by participant slot i
            let stride4 = i * 4;
            participants.push(ParticipantData {
                name,
                car_name,
                car_class,
                is_active,
                is_player: i as i32 == viewed_idx,
                race_position: ru32(ptr, base + P_RACE_POS),
                laps_completed: ru32(ptr, base + P_LAPS_DONE),
                current_lap: ru32(ptr, base + P_CUR_LAP),
                current_lap_distance: rf32(ptr, base + P_CUR_LAP_DIST),
                cur_s1: rf32(ptr, OFF_CUR_S1 + stride4),
                cur_s2: rf32(ptr, OFF_CUR_S2 + stride4),
                cur_s3: rf32(ptr, OFF_CUR_S3 + stride4),
                best_s1: rf32(ptr, OFF_BEST_S1 + stride4),
                best_s2: rf32(ptr, OFF_BEST_S2 + stride4),
                best_s3: rf32(ptr, OFF_BEST_S3 + stride4),
                fastest_lap_time: rf32(ptr, OFF_FASTEST_LAP_TIMES + stride4),
                last_lap_time: rf32(ptr, OFF_LAST_LAP_TIMES + stride4),
                world_pos_x: rf32(ptr, base + P_WORLD_POS),
                world_pos_z: rf32(ptr, base + P_WORLD_POS + 8),
                interval_gap_secs: 0.0,
                interval_gap_laps: 0,
                in_pits: ri32(ptr, base + P_CURRENT_SECTOR) < 0,
            });
        }
        // Sort by race position; unset (0) positions go to the end
        participants.sort_by(|a, b| match (a.race_position, b.race_position) {
            (0, 0) => std::cmp::Ordering::Equal,
            (0, _) => std::cmp::Ordering::Greater,
            (_, 0) => std::cmp::Ordering::Less,
            (x, y) => x.cmp(&y),
        });

        // ── Race interval (gap to car directly ahead) ─────────────────────────
        // Only meaningful during a race session (session_state == 5).
        if session_state == 5 && track_length > 0.0 {
            // Total distance covered: laps + fractional lap from current_lap_distance.
            let distances: Vec<f32> = participants
                .iter()
                .map(|p| {
                    let frac = (p.current_lap_distance / track_length).clamp(0.0, 1.0);
                    p.laps_completed as f32 + frac
                })
                .collect();

            // Leader's reference lap time for converting fraction → seconds.
            let ref_lap = participants
                .first()
                .map(|p| {
                    if p.fastest_lap_time > 0.0 {
                        p.fastest_lap_time
                    } else if p.last_lap_time > 0.0 {
                        p.last_lap_time
                    } else {
                        0.0
                    }
                })
                .unwrap_or(0.0);

            for i in 0..participants.len() {
                if i == 0 {
                    participants[i].interval_gap_secs = -1.0; // sentinel: leader
                } else {
                    let gap_dist = (distances[i - 1] - distances[i]).max(0.0);
                    let gap_laps = gap_dist.floor() as u32;
                    let gap_frac = gap_dist - gap_laps as f32;
                    participants[i].interval_gap_laps = gap_laps;
                    participants[i].interval_gap_secs = if ref_lap > 0.0 {
                        gap_frac * ref_lap
                    } else {
                        0.0
                    };
                }
            }
        }

        let race_flag_colour = ru32(ptr, OFF_HIGHEST_FLAG_COLOUR);
        let race_flag_reason = ru32(ptr, OFF_HIGHEST_FLAG_REASON);
        let pit_mode = ru32(ptr, OFF_PIT_MODE);

        UnmapViewOfFile(mapped);
        CloseHandle(handle);

        LiveSessionData {
            connected: true,
            game_state,
            session_state,
            race_state,
            num_participants,
            track_location,
            track_variation,
            track_length,
            laps_in_event,
            car_name,
            car_class,
            participants,
            player_telemetry,
            race_flag_colour,
            race_flag_reason,
            pit_mode,
        }
    }
}

#[cfg(not(windows))]
pub fn read_live_session() -> LiveSessionData {
    disconnected()
}
