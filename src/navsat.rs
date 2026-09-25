// Copyright 2025 Au-Zone Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! NavSat message creation and timestamp utilities.

use edgefirst_schemas::{
    builtin_interfaces,
    cdr::CdrError,
    sensor_msgs::{nav_sat_fix, nav_sat_status, NavSatFix, NavSatStatus},
};
use gpsd_proto::{GpsdError, Gst, ResponseData, Tpv};
use std::{
    collections::VecDeque,
    io::{self, Read},
    time::{Duration, SystemTime, SystemTimeError, UNIX_EPOCH},
};
use zenoh::time::{Timestamp, TimestampId, NTP64};

/// Errors that can occur when generating timestamps.
#[derive(Clone, Debug)]
pub enum TimestampError {
    /// System clock is before Unix epoch.
    BeforeEpoch(SystemTimeError),
    /// System clock seconds exceed i32 range (Y2038).
    Overflow,
}

impl std::fmt::Display for TimestampError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BeforeEpoch(e) => write!(f, "system clock before Unix epoch: {e}"),
            Self::Overflow => write!(f, "system clock seconds exceed i32 range"),
        }
    }
}

impl std::error::Error for TimestampError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BeforeEpoch(e) => Some(e),
            Self::Overflow => None,
        }
    }
}

/// Creates a NavSatFix message from TPV (Time-Position-Velocity) data.
///
/// # Arguments
///
/// * `tpv` - The TPV data from GPSD containing position information
/// * `stamp` - The timestamp to use for the message header
///
/// # Returns
///
/// A NavSatFix message populated with the TPV data.
pub fn create_navsat_fix_from_tpv(
    tpv: &Tpv,
    stamp: builtin_interfaces::Time,
) -> Result<NavSatFix<Vec<u8>>, CdrError> {
    NavSatFix::builder()
        .stamp(stamp)
        .frame_id("")
        .status(NavSatStatus {
            status: nav_sat_status::STATUS_FIX,
            service: nav_sat_status::SERVICE_GPS as u16,
        })
        .latitude(tpv.lat.unwrap_or(0.0))
        .longitude(tpv.lon.unwrap_or(0.0))
        .altitude(tpv.alt.unwrap_or(0.0) as f64)
        .position_covariance([-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        .position_covariance_type(nav_sat_fix::COVARIANCE_TYPE_UNKNOWN)
        .build()
}

/// Creates a NavSatFix message from GST (GPS Pseudorange Noise Statistics)
/// data.
///
/// # Arguments
///
/// * `gst` - The GST data from GPSD containing error estimates
/// * `stamp` - The timestamp to use for the message header
///
/// # Returns
///
/// A NavSatFix message populated with the GST data.
pub fn create_navsat_fix_from_gst(
    gst: &Gst,
    stamp: builtin_interfaces::Time,
) -> Result<NavSatFix<Vec<u8>>, CdrError> {
    NavSatFix::builder()
        .stamp(stamp)
        .frame_id("")
        .status(NavSatStatus {
            status: nav_sat_status::STATUS_FIX,
            service: nav_sat_status::SERVICE_GPS as u16,
        })
        .latitude(gst.lat.unwrap_or(0.0) as f64)
        .longitude(gst.lon.unwrap_or(0.0) as f64)
        .altitude(gst.alt.unwrap_or(0.0) as f64)
        .position_covariance([-1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
        .position_covariance_type(nav_sat_fix::COVARIANCE_TYPE_UNKNOWN)
        .build()
}

/// Gets the current wall-clock timestamp.
///
/// Uses `SystemTime` (backed by `CLOCK_REALTIME` on Linux) for ROS 2
/// compatible Header stamps. Wall-clock time is the convention across
/// the ROS 2 ecosystem, enabling correlation with logs, rosbags, and
/// external systems.
///
/// # Returns
///
/// A `Time` struct with seconds and nanoseconds, or an error if the
/// system clock is before the Unix epoch or beyond the i32 range (Y2038).
pub fn timestamp() -> Result<builtin_interfaces::Time, TimestampError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(TimestampError::BeforeEpoch)?;

    let secs = duration.as_secs();
    if secs > i32::MAX as u64 {
        return Err(TimestampError::Overflow);
    }

    Ok(builtin_interfaces::Time {
        sec: secs as i32,
        nanosec: duration.subsec_nanos(),
    })
}

/// Reception stamp of a GPSD line, as returned by [`timestamp`].
pub type Stamp = Result<builtin_interfaces::Time, TimestampError>;

/// Size of each read from the GPSD socket; GPSD JSON lines are well under this.
const READ_CHUNK: usize = 8192;

/// Line reader that stamps bytes when they are read from the transport.
///
/// Each `read()` call is stamped as it returns and the stamp is kept with the
/// bytes it delivered. A line takes the stamp of the read that delivered its
/// first byte, so lines buffered behind earlier ones keep their reception time
/// no matter how long the earlier lines take to parse and publish.
pub struct StampedLineReader<R> {
    inner: R,
    buf: Vec<u8>,
    /// Length and stamp of each read still (partly) held in `buf`, oldest first.
    chunks: VecDeque<(usize, Stamp)>,
}

impl<R: Read> StampedLineReader<R> {
    /// Creates a reader over `inner`.
    ///
    /// `pending` holds bytes already taken from the transport by an earlier
    /// reader (such as the `BufReader` used for the GPSD handshake). They are
    /// stamped now, the earliest instant this reader can attribute to them.
    pub fn new(inner: R, pending: Vec<u8>) -> Self {
        let mut chunks = VecDeque::new();
        if !pending.is_empty() {
            chunks.push_back((pending.len(), timestamp()));
        }
        Self {
            inner,
            buf: pending,
            chunks,
        }
    }

    /// Reads the next line, including its newline, into `line`.
    ///
    /// Returns the line's reception stamp, or `None` at end of stream with no
    /// bytes left. A final line without a newline is returned as is.
    pub fn read_line(&mut self, line: &mut Vec<u8>) -> io::Result<Option<Stamp>> {
        line.clear();
        let mut scanned = 0;
        loop {
            if let Some(pos) = self.buf[scanned..].iter().position(|&b| b == b'\n') {
                return Ok(Some(self.take(scanned + pos + 1, line)));
            }
            scanned = self.buf.len();

            let old_len = self.buf.len();
            self.buf.resize(old_len + READ_CHUNK, 0);
            let n = match self.inner.read(&mut self.buf[old_len..]) {
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                    self.buf.truncate(old_len);
                    continue;
                }
                Err(e) => {
                    self.buf.truncate(old_len);
                    return Err(e);
                }
            };
            let stamp = timestamp();
            self.buf.truncate(old_len + n);

            if n == 0 {
                if self.buf.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(self.take(self.buf.len(), line)));
            }
            self.chunks.push_back((n, stamp));
        }
    }

    /// Moves the first `len` bytes of `buf` into `line` and returns the stamp
    /// of the read that delivered the first of them.
    fn take(&mut self, len: usize, line: &mut Vec<u8>) -> Stamp {
        let stamp = self
            .chunks
            .front()
            .map(|(_, stamp)| stamp.clone())
            .expect("buffered bytes always have a chunk");
        line.extend(self.buf.drain(..len));

        let mut remaining = len;
        while remaining > 0 {
            let front = self.chunks.front_mut().expect("chunks cover buf");
            if front.0 > remaining {
                front.0 -= remaining;
                break;
            }
            remaining -= front.0;
            self.chunks.pop_front();
        }
        stamp
    }
}

/// Reads one GPSD JSON line and returns it with its reception stamp.
///
/// Equivalent to `gpsd_proto::get_data` apart from the stamp, which is taken
/// by `reader` when the bytes left the socket, before any parsing. At end of
/// stream the empty line fails to parse, as with `get_data`. `line` is a
/// reusable buffer.
pub fn read_response<R: Read>(
    reader: &mut StampedLineReader<R>,
    line: &mut Vec<u8>,
) -> Result<(ResponseData, Stamp), GpsdError> {
    let stamp = reader.read_line(line)?.unwrap_or_else(timestamp);
    let msg = serde_json::from_slice(line)?;
    Ok((msg, stamp))
}

/// Builds the Zenoh sample timestamp carrying the same instant as `stamp`.
///
/// Publishing `header.stamp` as the Zenoh timestamp makes the recorder's MCAP
/// `publish_time` equal the acquisition time in the CDR header. NTP64 is 32.32
/// fixed point, so the round trip is exact only to about 1 ns. Negative
/// seconds saturate to the Unix epoch.
pub fn zenoh_timestamp(stamp: &builtin_interfaces::Time, id: TimestampId) -> Timestamp {
    let duration = match u64::try_from(stamp.sec) {
        Ok(sec) => Duration::new(sec, stamp.nanosec),
        Err(_) => Duration::ZERO,
    };
    Timestamp::new(NTP64::from(duration), id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpsd_proto::Mode;

    /// Helper to create a Tpv with optional position data
    fn make_tpv(lat: Option<f64>, lon: Option<f64>, alt: Option<f32>) -> Tpv {
        Tpv {
            device: None,
            status: None,
            mode: Mode::NoFix,
            time: None,
            ept: None,
            leapseconds: None,
            alt_msl: None,
            alt_hae: None,
            geoid_sep: None,
            lat,
            lon,
            alt,
            epx: None,
            epy: None,
            epv: None,
            track: None,
            speed: None,
            climb: None,
            epd: None,
            eps: None,
            epc: None,
            eph: None,
        }
    }

    /// Helper to create a Gst with optional position data
    fn make_gst(lat: Option<f32>, lon: Option<f32>, alt: Option<f32>) -> Gst {
        Gst {
            device: None,
            time: None,
            rms: None,
            major: None,
            minor: None,
            orient: None,
            lat,
            lon,
            alt,
        }
    }

    #[test]
    fn test_timestamp_matches_system_time() {
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let time = timestamp().unwrap();
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

        // Timestamp should fall between the two SystemTime samples
        let ts_secs = time.sec as u64;
        assert!(ts_secs >= before.as_secs());
        assert!(ts_secs <= after.as_secs());
        assert!(time.nanosec < 1_000_000_000);
    }

    fn test_id() -> TimestampId {
        TimestampId::try_from(1u8).unwrap()
    }

    #[test]
    fn test_zenoh_timestamp_matches_stamp() {
        for nanosec in [0, 1, 123_456_789, 999_999_999] {
            let stamp = builtin_interfaces::Time {
                sec: 1_758_800_000,
                nanosec,
            };
            let ts = zenoh_timestamp(&stamp, test_id());
            let decoded = ts.get_time().to_duration();

            // NTP64 quantizes to 2^-32 s, so compare within 2 ns
            let expected = Duration::new(stamp.sec as u64, stamp.nanosec);
            let diff = decoded.abs_diff(expected);
            assert!(
                diff <= Duration::from_nanos(2),
                "nanosec {nanosec}: decoded {decoded:?} differs from {expected:?} by {diff:?}"
            );
            assert_eq!(ts.get_id(), &test_id());
        }
    }

    #[test]
    fn test_zenoh_timestamp_saturated_stamp() {
        let stamp = builtin_interfaces::Time {
            sec: i32::MAX,
            nanosec: 999_999_999,
        };
        let decoded = zenoh_timestamp(&stamp, test_id()).get_time().to_duration();
        assert_eq!(decoded.as_secs(), i32::MAX as u64);
    }

    #[test]
    fn test_zenoh_timestamp_negative_saturates_to_epoch() {
        let stamp = builtin_interfaces::Time {
            sec: -5,
            nanosec: 500,
        };
        let decoded = zenoh_timestamp(&stamp, test_id()).get_time().to_duration();
        assert_eq!(decoded, Duration::ZERO);
    }

    const TPV_LINE: &[u8] = b"{\"class\":\"TPV\",\"mode\":3,\"lat\":66.123}\r\n";
    const PPS_LINE: &[u8] = b"{\"class\":\"PPS\",\"device\":\"/dev/pps0\",\"real_sec\":1,\"real_nsec\":2,\"clock_sec\":3,\"clock_nsec\":4,\"precision\":-20}\r\n";

    fn stamp_duration(stamp: &Stamp) -> Duration {
        let t = stamp.as_ref().unwrap();
        Duration::new(t.sec as u64, t.nanosec)
    }

    fn now() -> Duration {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
    }

    /// Returns one scripted chunk per `read()` and records when each read began.
    struct ChunkReader {
        chunks: VecDeque<io::Result<Vec<u8>>>,
        read_started: Vec<Duration>,
    }

    impl ChunkReader {
        fn new(chunks: Vec<io::Result<Vec<u8>>>) -> Self {
            Self {
                chunks: chunks.into(),
                read_started: Vec::new(),
            }
        }
    }

    impl Read for ChunkReader {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            // Keep consecutive reads distinguishable on the wall clock
            std::thread::sleep(Duration::from_millis(2));
            self.read_started.push(now());
            match self.chunks.pop_front() {
                None => Ok(0),
                Some(Err(e)) => Err(e),
                Some(Ok(chunk)) => {
                    out[..chunk.len()].copy_from_slice(&chunk);
                    Ok(chunk.len())
                }
            }
        }
    }

    #[test]
    fn test_read_response_parses_and_stamps_line() {
        let mut reader = StampedLineReader::new(TPV_LINE, Vec::new());
        let mut line = Vec::new();

        let before = now();
        let (msg, stamp) = read_response(&mut reader, &mut line).unwrap();
        let after = now();

        match msg {
            ResponseData::Tpv(tpv) => assert_eq!(tpv.lat, Some(66.123)),
            other => panic!("expected TPV, got {other:?}"),
        }
        let stamp = stamp_duration(&stamp);
        assert!(before <= stamp && stamp <= after);
    }

    #[test]
    fn test_buffered_line_keeps_stamp_of_its_read() {
        let burst = [TPV_LINE, PPS_LINE].concat();
        let mut reader = StampedLineReader::new(ChunkReader::new(vec![Ok(burst)]), Vec::new());
        let mut line = Vec::new();

        let (msg, first) = read_response(&mut reader, &mut line).unwrap();
        assert!(matches!(msg, ResponseData::Tpv(_)));

        // Parsing and publishing the first line must not move the second's stamp
        std::thread::sleep(Duration::from_millis(5));
        let (msg, second) = read_response(&mut reader, &mut line).unwrap();
        assert!(matches!(msg, ResponseData::Pps(_)));
        assert_eq!(stamp_duration(&first), stamp_duration(&second));
    }

    #[test]
    fn test_line_spanning_reads_takes_first_read_stamp() {
        let (tpv_head, tpv_tail) = TPV_LINE.split_at(10);
        let (pps_head, pps_tail) = PPS_LINE.split_at(10);
        let mut reader = StampedLineReader::new(
            ChunkReader::new(vec![
                Ok(tpv_head.to_vec()),
                Ok([tpv_tail, pps_head].concat()),
                Ok(pps_tail.to_vec()),
            ]),
            Vec::new(),
        );
        let mut line = Vec::new();

        let tpv = reader.read_line(&mut line).unwrap().unwrap();
        assert_eq!(line, TPV_LINE);
        let pps = reader.read_line(&mut line).unwrap().unwrap();
        assert_eq!(line, PPS_LINE);

        // Each line is stamped by the read that delivered its first byte
        let started = &reader.inner.read_started;
        assert_eq!(started.len(), 3);
        let (tpv, pps) = (stamp_duration(&tpv), stamp_duration(&pps));
        assert!(started[0] <= tpv && tpv < started[1]);
        assert!(started[1] <= pps && pps < started[2]);
    }

    #[test]
    fn test_pending_bytes_are_read_before_transport() {
        let mut reader = StampedLineReader::new(PPS_LINE, TPV_LINE.to_vec());
        let mut line = Vec::new();

        let (msg, _) = read_response(&mut reader, &mut line).unwrap();
        assert!(matches!(msg, ResponseData::Tpv(_)));
        let (msg, _) = read_response(&mut reader, &mut line).unwrap();
        assert!(matches!(msg, ResponseData::Pps(_)));
    }

    #[test]
    fn test_read_line_end_of_stream() {
        let mut reader = StampedLineReader::new(&b"partial"[..], Vec::new());
        let mut line = Vec::new();

        assert!(reader.read_line(&mut line).unwrap().is_some());
        assert_eq!(line, b"partial");
        assert!(reader.read_line(&mut line).unwrap().is_none());
        assert!(line.is_empty());
    }

    #[test]
    fn test_read_line_retries_interrupted_read() {
        let mut reader = StampedLineReader::new(
            ChunkReader::new(vec![
                Err(io::ErrorKind::Interrupted.into()),
                Ok(TPV_LINE.to_vec()),
            ]),
            Vec::new(),
        );
        let mut line = Vec::new();

        assert!(reader.read_line(&mut line).unwrap().is_some());
        assert_eq!(line, TPV_LINE);
    }

    #[test]
    fn test_read_line_propagates_io_error() {
        let mut reader = StampedLineReader::new(
            ChunkReader::new(vec![Err(io::ErrorKind::ConnectionReset.into())]),
            Vec::new(),
        );
        let mut line = Vec::new();

        let err = reader.read_line(&mut line).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::ConnectionReset);
    }

    #[test]
    fn test_read_response_rejects_invalid_json() {
        let mut reader = StampedLineReader::new(&b"not json\n"[..], Vec::new());
        let mut line = Vec::new();
        assert!(matches!(
            read_response(&mut reader, &mut line),
            Err(GpsdError::JsonError(_))
        ));
    }

    #[test]
    fn test_timestamp_consecutive_calls_valid() {
        let time1 = timestamp().unwrap();
        let time2 = timestamp().unwrap();

        // Both should have valid nanosecond range
        assert!(time1.nanosec < 1_000_000_000);
        assert!(time2.nanosec < 1_000_000_000);

        // Both should have non-negative seconds (valid post-epoch)
        assert!(time1.sec >= 0);
        assert!(time2.sec >= 0);
    }

    #[test]
    fn test_create_navsat_fix_from_tpv_with_data() {
        let tpv = make_tpv(Some(45.4215), Some(-75.6972), Some(100.0));

        let stamp = builtin_interfaces::Time {
            sec: 123,
            nanosec: 456,
        };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp).expect("valid NavSatFix");

        assert_eq!(msg.latitude(), 45.4215);
        assert_eq!(msg.longitude(), -75.6972);
        assert_eq!(msg.altitude(), 100.0);
        assert_eq!(msg.stamp().sec, 123);
        assert_eq!(msg.stamp().nanosec, 456);
        assert_eq!(msg.status().status, nav_sat_status::STATUS_FIX);
        assert_eq!(msg.status().service, nav_sat_status::SERVICE_GPS as u16);
        assert_eq!(
            msg.position_covariance_type(),
            nav_sat_fix::COVARIANCE_TYPE_UNKNOWN
        );
    }

    #[test]
    fn test_create_navsat_fix_from_tpv_with_none_values() {
        let tpv = make_tpv(None, None, None);

        let stamp = builtin_interfaces::Time { sec: 0, nanosec: 0 };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp).expect("valid NavSatFix");

        assert_eq!(msg.latitude(), 0.0);
        assert_eq!(msg.longitude(), 0.0);
        assert_eq!(msg.altitude(), 0.0);
    }

    #[test]
    fn test_create_navsat_fix_from_gst_with_data() {
        let gst = make_gst(Some(45.4215), Some(-75.6972), Some(100.0));

        let stamp = builtin_interfaces::Time {
            sec: 789,
            nanosec: 101112,
        };
        let msg = create_navsat_fix_from_gst(&gst, stamp).expect("valid NavSatFix");

        assert_eq!(msg.latitude() as f32, 45.4215);
        assert_eq!(msg.longitude() as f32, -75.6972);
        assert_eq!(msg.altitude(), 100.0);
        assert_eq!(msg.stamp().sec, 789);
        assert_eq!(msg.stamp().nanosec, 101112);
    }

    #[test]
    fn test_create_navsat_fix_from_gst_with_none_values() {
        let gst = make_gst(None, None, None);

        let stamp = builtin_interfaces::Time { sec: 0, nanosec: 0 };
        let msg = create_navsat_fix_from_gst(&gst, stamp).expect("valid NavSatFix");

        assert_eq!(msg.latitude(), 0.0);
        assert_eq!(msg.longitude(), 0.0);
        assert_eq!(msg.altitude(), 0.0);
    }

    #[test]
    fn test_navsat_fix_covariance_is_unknown() {
        let tpv = make_tpv(None, None, None);
        let stamp = builtin_interfaces::Time { sec: 0, nanosec: 0 };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp).expect("valid NavSatFix");

        assert_eq!(msg.position_covariance()[0], -1.0);
        assert_eq!(
            msg.position_covariance_type(),
            nav_sat_fix::COVARIANCE_TYPE_UNKNOWN
        );
    }

    #[test]
    fn test_create_navsat_fix_header_frame_id_is_empty() {
        let tpv = make_tpv(Some(0.0), Some(0.0), Some(0.0));
        let stamp = builtin_interfaces::Time { sec: 0, nanosec: 0 };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp).expect("valid NavSatFix");

        assert!(msg.frame_id().is_empty());
    }

    #[test]
    fn navsat_fix_cdr_roundtrip() {
        let tpv = make_tpv(Some(45.4215), Some(-75.6972), Some(100.0));
        let stamp = builtin_interfaces::Time {
            sec: 123,
            nanosec: 456,
        };
        let msg = create_navsat_fix_from_tpv(&tpv, stamp).expect("valid NavSatFix");
        let decoded = NavSatFix::from_cdr(msg.into_cdr()).expect("decode NavSatFix");
        assert_eq!(decoded.latitude(), 45.4215);
        assert_eq!(decoded.longitude(), -75.6972);
        assert_eq!(decoded.altitude(), 100.0);
        assert_eq!(decoded.stamp().sec, 123);
        assert_eq!(decoded.stamp().nanosec, 456);
        assert_eq!(decoded.status().status, nav_sat_status::STATUS_FIX);
    }

    /// Hardware integration tests that require real GPS hardware.
    /// These tests are marked with #[ignore] and only run on hardware runners.
    /// Execute with: cargo test -- --ignored --include-ignored
    #[cfg(test)]
    mod hardware_tests {
        use super::*;
        use gpsd_proto::{get_data, handshake, Mode, ResponseData};
        use std::{
            io::{BufReader, BufWriter},
            net::TcpStream,
            time::Duration,
        };

        /// GPS metrics collected during hardware tests
        #[derive(Debug, Default)]
        struct GpsMetrics {
            fix_mode: Option<Mode>,
            satellites_used: usize,
            satellites_visible: usize,
            latitude: Option<f64>,
            longitude: Option<f64>,
            altitude: Option<f64>,
            hdop: Option<f32>,
            vdop: Option<f32>,
            pdop: Option<f32>,
            max_snr: Option<f32>,
            avg_snr: Option<f32>,
        }

        impl GpsMetrics {
            fn print_summary(&self) {
                println!("\n=== GPS Hardware Test Metrics ===");
                if let Some(mode) = &self.fix_mode {
                    let fix_type = match mode {
                        Mode::NoFix => "No Fix",
                        Mode::Fix2d => "2D Fix",
                        Mode::Fix3d => "3D Fix",
                    };
                    println!("Fix Quality: {}", fix_type);
                }
                println!(
                    "Satellites: {} used / {} visible",
                    self.satellites_used, self.satellites_visible
                );

                if let (Some(lat), Some(lon), Some(alt)) =
                    (self.latitude, self.longitude, self.altitude)
                {
                    println!("Position: {:.6}°, {:.6}° @ {:.1}m", lat, lon, alt);
                }

                if let (Some(h), Some(v), Some(p)) = (self.hdop, self.vdop, self.pdop) {
                    println!("DOP: HDOP={:.2} VDOP={:.2} PDOP={:.2}", h, v, p);
                }

                if let Some(max_snr) = self.max_snr {
                    println!("SNR: max={:.1} dB", max_snr);
                    if let Some(avg_snr) = self.avg_snr {
                        println!("     avg={:.1} dB", avg_snr);
                    }
                }
                println!("=================================\n");
            }
        }

        fn collect_gps_metrics(timeout_secs: u64) -> Result<GpsMetrics, String> {
            let stream = TcpStream::connect("127.0.0.1:2947")
                .map_err(|e| format!("Failed to connect to GPSD: {}", e))?;

            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .map_err(|e| format!("Failed to set timeout: {}", e))?;

            let mut reader = BufReader::new(&stream);
            let mut writer = BufWriter::new(&stream);

            handshake(&mut reader, &mut writer)
                .map_err(|e| format!("GPSD handshake failed: {}", e))?;

            let mut metrics = GpsMetrics::default();
            let start = std::time::Instant::now();

            while start.elapsed().as_secs() < timeout_secs {
                match get_data(&mut reader) {
                    Ok(ResponseData::Tpv(tpv)) => {
                        // Only update fix mode if it's better than current
                        // This prevents transient NoFix from overwriting a good fix
                        let dominated = matches!(
                            (&metrics.fix_mode, &tpv.mode),
                            (None, _) | (Some(Mode::NoFix), _) | (Some(Mode::Fix2d), Mode::Fix3d)
                        );
                        if dominated {
                            metrics.fix_mode = Some(tpv.mode);
                        }

                        // Update position data if available
                        if tpv.lat.is_some() {
                            metrics.latitude = tpv.lat;
                        }
                        if tpv.lon.is_some() {
                            metrics.longitude = tpv.lon;
                        }
                        if tpv.alt.is_some() {
                            metrics.altitude = tpv.alt.map(|a| a as f64);
                        }
                    }
                    Ok(ResponseData::Sky(sky)) => {
                        if let Some(sats) = &sky.satellites {
                            metrics.satellites_visible = sats.len();
                            metrics.satellites_used = sats.iter().filter(|s| s.used).count();

                            let snr_values: Vec<f32> = sats.iter().filter_map(|s| s.ss).collect();

                            if !snr_values.is_empty() {
                                metrics.max_snr = snr_values
                                    .iter()
                                    .copied()
                                    .max_by(|a, b| a.partial_cmp(b).unwrap());
                                let sum: f32 = snr_values.iter().sum();
                                metrics.avg_snr = Some(sum / snr_values.len() as f32);
                            }
                        }

                        // Store DOP values if available
                        if let Some(h) = sky.hdop {
                            metrics.hdop = Some(h);
                        }
                        if let Some(v) = sky.vdop {
                            metrics.vdop = Some(v);
                        }
                        if let Some(p) = sky.pdop {
                            metrics.pdop = Some(p);
                        }
                    }
                    Ok(ResponseData::Gst(_gst)) => {
                        // GST provides error estimates but not DOP
                        // Could extract lat_err, lon_err, alt_err if needed
                    }
                    Ok(_) => {
                        // Other message types
                    }
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                }

                // Only exit early if we have a valid fix (2D or 3D) AND satellite data
                // Don't exit early on NoFix - keep collecting until we get a good fix or
                // timeout
                let has_valid_fix =
                    matches!(metrics.fix_mode, Some(Mode::Fix2d) | Some(Mode::Fix3d));
                if has_valid_fix && metrics.satellites_visible > 0 && metrics.latitude.is_some() {
                    break;
                }
            }

            Ok(metrics)
        }

        /// Test GPSD connection on real hardware
        #[test]
        #[ignore = "Requires GPSD daemon and GPS hardware"]
        fn test_gpsd_connection() {
            let result = TcpStream::connect("127.0.0.1:2947");
            assert!(
                result.is_ok(),
                "Failed to connect to GPSD. Is gpsd daemon running?"
            );
        }

        /// Test GPS receiver has valid 3D fix with good signal quality
        #[test]
        #[ignore = "Requires GPSD daemon and GPS hardware"]
        fn test_gps_fix_quality() {
            let metrics = collect_gps_metrics(30).expect("Failed to collect GPS metrics");

            metrics.print_summary();

            // Require 3D fix
            assert!(
                matches!(metrics.fix_mode, Some(Mode::Fix3d)),
                "Expected 3D fix, got {:?}. Is GPS antenna positioned correctly?",
                metrics.fix_mode
            );

            // Require at least 4 satellites for 3D fix
            assert!(
                metrics.satellites_used >= 4,
                "Expected at least 4 satellites for 3D fix, got {}. Check antenna placement.",
                metrics.satellites_used
            );

            // Require position data
            assert!(
                metrics.latitude.is_some() && metrics.longitude.is_some(),
                "No position data received. GPS may not have achieved fix."
            );

            // Check altitude is reasonable (not default 0)
            if let Some(alt) = metrics.altitude {
                assert!(
                    alt.abs() > 1.0 || alt == 0.0,
                    "Altitude appears invalid: {}m",
                    alt
                );
            }
        }

        /// Test GPS signal quality (SNR) is sufficient
        #[test]
        #[ignore = "Requires GPSD daemon and GPS hardware"]
        fn test_gps_signal_quality() {
            let metrics = collect_gps_metrics(30).expect("Failed to collect GPS metrics");

            metrics.print_summary();

            // Require satellites visible
            assert!(
                metrics.satellites_visible > 0,
                "No satellites visible. Check GPS antenna connection and sky view."
            );

            // Check SNR if available
            if let Some(max_snr) = metrics.max_snr {
                assert!(
                    max_snr > 20.0,
                    "Maximum SNR too low: {:.1} dB. Expected > 20 dB. Check antenna.",
                    max_snr
                );

                if let Some(avg_snr) = metrics.avg_snr {
                    assert!(
                        avg_snr > 12.0,
                        "Average SNR too low: {:.1} dB. Expected > 12 dB. Weak signal.",
                        avg_snr
                    );
                }
            } else {
                panic!("No SNR data available from satellites");
            }
        }

        /// Test position reporting - captures actual GPS coordinates
        #[test]
        #[ignore = "Requires GPSD daemon and GPS hardware"]
        fn test_gps_position_reporting() {
            let metrics = collect_gps_metrics(30).expect("Failed to collect GPS metrics");

            metrics.print_summary();

            // Validate we have position
            let (lat, lon) = match (metrics.latitude, metrics.longitude) {
                (Some(lat), Some(lon)) => (lat, lon),
                _ => panic!("No GPS position available after 30 seconds"),
            };

            // Sanity check: coordinates should be valid ranges
            assert!((-90.0..=90.0).contains(&lat), "Invalid latitude: {}", lat);
            assert!(
                (-180.0..=180.0).contains(&lon),
                "Invalid longitude: {}",
                lon
            );

            println!("GPS Test Location: {:.6}°, {:.6}°", lat, lon);

            if let Some(alt) = metrics.altitude {
                println!("GPS Test Altitude: {:.1}m", alt);
            }
        }

        /// Test wall-clock timestamp generation on real hardware
        #[test]
        #[ignore = "Requires real-time clock on hardware"]
        fn test_hardware_timestamp_accuracy() {
            let time1 = timestamp().expect("Failed to get hardware timestamp");
            let mono_start = std::time::Instant::now();
            std::thread::sleep(std::time::Duration::from_millis(100));
            let time2 = timestamp().expect("Failed to get second hardware timestamp");
            let mono_elapsed = mono_start.elapsed();

            // Wall-clock times should be valid (past 2020)
            assert!(
                time1.sec > 1_577_836_800,
                "Wall-clock time looks invalid (pre-2020): {}",
                time1.sec
            );
            assert!(
                time2.sec > 1_577_836_800,
                "Wall-clock time looks invalid (pre-2020): {}",
                time2.sec
            );

            // Use monotonic clock to verify sleep duration (not wall-clock delta)
            let elapsed_ms = mono_elapsed.as_millis();
            assert!(
                (50..=200).contains(&elapsed_ms),
                "Monotonic elapsed time {}ms not in expected range [50-200ms]",
                elapsed_ms
            );

            println!(
                "Wall-clock timestamps: t1.sec={}, t2.sec={} (monotonic elapsed: {}ms)",
                time1.sec, time2.sec, elapsed_ms
            );
        }
    }
}
