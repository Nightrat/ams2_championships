use serde::Serialize;

/// Real-time participant data extracted from AMS2 shared memory.
#[derive(Serialize, Clone)]
pub struct ParticipantData {
    pub name: String,
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
    /// Total track length in metres (mTrackLength), used for gap calculation.
    pub track_length: f32,
    pub participants: Vec<ParticipantData>,
}

fn disconnected() -> LiveSessionData {
    LiveSessionData {
        connected: false,
        game_state: 0,
        session_state: 0,
        race_state: 0,
        num_participants: 0,
        track_location: String::new(),
        track_length: 0.0,
        participants: vec![],
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
    const OFF_TRACK_LOCATION: usize = 6576;
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
    const P_CUR_LAP_DIST: usize = 80;
    const P_RACE_POS: usize = 84;
    const P_LAPS_DONE: usize = 88;
    const P_CUR_LAP: usize = 92;

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

        let game_state = ru32(ptr, OFF_GAME_STATE);
        let session_state = ru32(ptr, OFF_SESSION_STATE);
        let race_state = ru32(ptr, OFF_RACE_STATE);
        let viewed_idx = ri32(ptr, OFF_VIEWED_PARTICIPANT);
        let num_participants = ri32(ptr, OFF_NUM_PARTICIPANTS).clamp(0, 64);
        let track_location = rstr(ptr, OFF_TRACK_LOCATION, 64);
        let track_length = rf32(ptr, OFF_TRACK_LENGTH);

        let mut participants = Vec::with_capacity(num_participants as usize);
        for i in 0..num_participants as usize {
            let base = OFF_PARTICIPANTS + i * STRIDE;
            let is_active = ru8(ptr, base + P_IS_ACTIVE) != 0;
            if !is_active {
                continue;
            }
            let name = rstr(ptr, base + P_NAME, 64);

            // All timing arrays are at top-level, indexed by participant slot i
            let stride4 = i * 4;
            participants.push(ParticipantData {
                name,
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
            track_length,
            participants,
        }
    }
}

#[cfg(not(windows))]
pub fn read_live_session() -> LiveSessionData {
    disconnected()
}
