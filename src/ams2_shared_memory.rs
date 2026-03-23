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
}

/// Player car telemetry from the AMS2 shared memory (player's car only).
/// Wheel order for all arrays: FL=0, FR=1, RL=2, RR=3.
#[derive(Serialize, Clone)]
pub struct PlayerTelemetry {
    /// Left-edge tyre temperature, °C.
    pub tyre_temp_left:    [f32; 4],
    /// Centre-tread tyre temperature, °C.
    pub tyre_temp_center:  [f32; 4],
    /// Right-edge tyre temperature, °C.
    pub tyre_temp_right:   [f32; 4],
    /// Tyre wear 0–1 (0 = new, 1 = fully worn).
    pub tyre_wear:         [f32; 4],
    /// Tyre air pressure, PSI.
    pub tyre_pressure:     [f32; 4],
    /// Brake disc temperature, °C.
    pub brake_temp:        [f32; 4],
    /// Suspension travel, metres.
    pub suspension_travel: [f32; 4],
    /// Ride height per corner, cm.
    pub ride_height:       [f32; 4],
    /// Filtered throttle 0–1.
    pub throttle:   f32,
    /// Filtered brake 0–1.
    pub brake_input: f32,
    /// Filtered steering −1…+1.
    pub steering:   f32,
    /// Speed m/s.
    pub speed:      f32,
    /// Engine RPM.
    pub rpm:        f32,
    /// Current gear (negative = reverse, 0 = neutral).
    pub gear:       i32,
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
    /// Player's car name (mCarName).
    pub car_name: String,
    /// Player's car class name (mCarClassName).
    pub car_class: String,
    pub participants: Vec<ParticipantData>,
    pub player_telemetry: PlayerTelemetry,
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
        car_name: String::new(),
        car_class: String::new(),
        participants: vec![],
        player_telemetry: PlayerTelemetry {
            tyre_temp_left:    [0.0; 4],
            tyre_temp_center:  [0.0; 4],
            tyre_temp_right:   [0.0; 4],
            tyre_wear:         [0.0; 4],
            tyre_pressure:     [0.0; 4],
            brake_temp:        [0.0; 4],
            suspension_travel: [0.0; 4],
            ride_height:       [0.0; 4],
            throttle: 0.0, brake_input: 0.0, steering: 0.0,
            speed: 0.0, rpm: 0.0, gear: 0,
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
    const OFF_CAR_NAME:        usize = 6444;  // char mCarName[64]
    const OFF_CAR_CLASS:       usize = 6508;  // char mCarClassName[64]
    const OFF_TRACK_LOCATION:  usize = 6576;  // char mTrackLocation[64]
    const OFF_TRACK_VARIATION: usize = 6640;  // char mTrackVariation[64]
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
    const OFF_CAR_NAMES:       usize = 11056; // char mCarNames[64][64]
    const OFF_CAR_CLASS_NAMES: usize = 15152; // char mCarClassNames[64][64]
    // ── Player car telemetry ──────────────────────────────────────────────────
    const OFF_SPEED:              usize = 6848;  // float mSpeed (m/s)
    const OFF_RPM:                usize = 6852;  // float mRpm
    const OFF_BRAKE_INPUT:        usize = 6860;  // float mBrake (filtered)
    const OFF_THROTTLE:           usize = 6864;  // float mThrottle (filtered)
    const OFF_STEERING:           usize = 6872;  // float mSteering (filtered)
    const OFF_GEAR:               usize = 6876;  // int mGear
    const OFF_TYRE_WEAR:          usize = 7136;  // float mTyreWear[4]
    const OFF_BRAKE_TEMP:         usize = 7184;  // float mBrakeTempCelsius[4]
    const OFF_SUSPENSION_TRAVEL:  usize = 7340;  // float mSuspensionTravel[4] (metres)
    const OFF_TYRE_PRESSURE:      usize = 7372;  // float mAirPressure[4] (PSI)
    // AMS2-specific additions (not in original PC2 header):
    const OFF_TYRE_TEMP_LEFT:     usize = 20584; // float mTyreTempLeft[4] (°C)
    const OFF_TYRE_TEMP_CENTER:   usize = 20600; // float mTyreTempCenter[4] (°C)
    const OFF_TYRE_TEMP_RIGHT:    usize = 20616; // float mTyreTempRight[4] (°C)
    const OFF_RIDE_HEIGHT:        usize = 20636; // float mRideHeight[4] (cm)
    const OFF_CUR_S1: usize = 7408;
    const OFF_CUR_S2: usize = 7664;
    const OFF_CUR_S3: usize = 7920;
    const OFF_BEST_S1: usize = 8176;
    const OFF_BEST_S2: usize = 8432;
    const OFF_BEST_S3: usize = 8688;
    const OFF_FASTEST_LAP_TIMES: usize = 8944;
    const OFF_LAST_LAP_TIMES: usize = 9200;

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

    unsafe fn rf32x4(b: *const u8, off: usize) -> [f32; 4] {
        [rf32(b, off), rf32(b, off + 4), rf32(b, off + 8), rf32(b, off + 12)]
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
            tyre_temp_left:    rf32x4(ptr, OFF_TYRE_TEMP_LEFT),
            tyre_temp_center:  rf32x4(ptr, OFF_TYRE_TEMP_CENTER),
            tyre_temp_right:   rf32x4(ptr, OFF_TYRE_TEMP_RIGHT),
            tyre_wear:         rf32x4(ptr, OFF_TYRE_WEAR),
            tyre_pressure:     rf32x4(ptr, OFF_TYRE_PRESSURE),
            brake_temp:        rf32x4(ptr, OFF_BRAKE_TEMP),
            suspension_travel: rf32x4(ptr, OFF_SUSPENSION_TRAVEL),
            ride_height:       rf32x4(ptr, OFF_RIDE_HEIGHT),
            throttle:    rf32(ptr, OFF_THROTTLE),
            brake_input: rf32(ptr, OFF_BRAKE_INPUT),
            steering:    rf32(ptr, OFF_STEERING),
            speed:       rf32(ptr, OFF_SPEED),
            rpm:         rf32(ptr, OFF_RPM),
            gear:        ri32(ptr, OFF_GEAR),
        };
        let game_state = ru32(ptr, OFF_GAME_STATE);
        let session_state = ru32(ptr, OFF_SESSION_STATE);
        let race_state = ru32(ptr, OFF_RACE_STATE);
        let viewed_idx = ri32(ptr, OFF_VIEWED_PARTICIPANT);
        let num_participants = ri32(ptr, OFF_NUM_PARTICIPANTS).clamp(0, 64);
        let car_name       = rstr(ptr, OFF_CAR_NAME, 64);
        let car_class      = rstr(ptr, OFF_CAR_CLASS, 64);
        let track_location  = rstr(ptr, OFF_TRACK_LOCATION, 64);
        let track_variation = rstr(ptr, OFF_TRACK_VARIATION, 64);
        let track_length    = rf32(ptr, OFF_TRACK_LENGTH);

        let mut participants = Vec::with_capacity(num_participants as usize);
        for i in 0..num_participants as usize {
            let base = OFF_PARTICIPANTS + i * STRIDE;
            let is_active = ru8(ptr, base + P_IS_ACTIVE) != 0;
            if !is_active {
                continue;
            }
            let name      = rstr(ptr, base + P_NAME, 64);
            let car_name  = rstr(ptr, OFF_CAR_NAMES + i * 64, 64);
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
            });
        }
        // Sort by race position; unset (0) positions go to the end
        participants.sort_by(|a, b| {
            match (a.race_position, b.race_position) {
                (0, 0) => std::cmp::Ordering::Equal,
                (0, _) => std::cmp::Ordering::Greater,
                (_, 0) => std::cmp::Ordering::Less,
                (x, y) => x.cmp(&y),
            }
        });

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
            car_name,
            car_class,
            participants,
            player_telemetry,
        }
    }
}

#[cfg(not(windows))]
pub fn read_live_session() -> LiveSessionData {
    disconnected()
}
