#!/usr/bin/env python3
"""
End-to-end Zenoh integration test for EdgeFirst NavSat service.

This test validates that the edgefirst-navsat service correctly publishes
NavSatFix messages over Zenoh that can be consumed by Python clients.

Requirements:
    - edgefirst-navsat service running on localhost
    - GPSD daemon with GPS receiver providing data
    - edgefirst-schemas Python package installed
    - pytest for test execution

Usage:
    pytest tests/test_zenoh_integration.py --junit-xml=test-results.xml -v
"""

import socket
import time
import pytest
import zenoh
from edgefirst.schemas.sensor_msgs import NavSatFix

# Test configuration. The service sets session namespace = hostname, so a
# namespace-free subscriber must use the wire key `{hostname}/gps`.
def _hostname():
    host = socket.gethostname()
    if not host or "/" in host:
        return "localhost"
    return host

ZENOH_TOPIC = f"{_hostname()}/gps"
TEST_TIMEOUT_SECONDS = 60
MIN_MESSAGES_REQUIRED = 5

# NavSatStatus.status constants from ROS2 sensor_msgs/msg/NavSatStatus
# Reference: https://docs.ros2.org/foxy/api/sensor_msgs/msg/NavSatStatus.html
#
# These match the constants defined in edgefirst-schemas (Rust and Python):
#   - STATUS_NO_FIX = -1  (unable to fix position)
#   - STATUS_FIX = 0      (unaugmented fix - basic GPS)
#   - STATUS_SBAS_FIX = 1 (satellite-based augmentation: WAAS/EGNOS)
#   - STATUS_GBAS_FIX = 2 (ground-based augmentation: RTK/DGPS)
#
# Note: A fix is valid when status >= STATUS_FIX (per ROS2 spec)
# Note: This is NOT the same as GPSD's "mode" field (NoFix/2D/3D)
STATUS_NO_FIX = -1
STATUS_FIX = 0
STATUS_SBAS_FIX = 1
STATUS_GBAS_FIX = 2


@pytest.fixture(scope="module")
def zenoh_session():
    """Create Zenoh session for testing."""
    config = zenoh.Config()
    config.insert_json5("scouting/multicast/interface", "'lo'")
    session = zenoh.open(config)
    yield session
    session.close()


class NavSatCollector:
    """Helper class to collect NavSatFix messages from Zenoh."""
    
    def __init__(self):
        self.messages = []
        self.errors = []
        
    def callback(self, sample):
        """Zenoh subscriber callback."""
        try:
            nav_fix = NavSatFix.deserialize(sample.payload.to_bytes())
            self.messages.append(nav_fix)
        except Exception as e:
            self.errors.append(str(e))
    
    def wait_for_messages(self, min_count, timeout_secs):
        """Wait until we have minimum message count or timeout."""
        start_time = time.time()
        while len(self.messages) < min_count:
            if time.time() - start_time > timeout_secs:
                return False
            time.sleep(0.1)
        return True


@pytest.mark.hardware
@pytest.mark.timeout(TEST_TIMEOUT_SECONDS + 10)
class TestZenohIntegration:
    """Zenoh end-to-end integration tests for NavSat service."""
    
    def test_service_publishes_messages(self, zenoh_session):
        """Test that edgefirst-navsat service publishes NavSatFix messages."""
        collector = NavSatCollector()
        
        # Subscribe to NavSatFix messages
        subscriber = zenoh_session.declare_subscriber(ZENOH_TOPIC, collector.callback)
        
        try:
            # Wait for messages
            success = collector.wait_for_messages(MIN_MESSAGES_REQUIRED, TEST_TIMEOUT_SECONDS)
            
            # Print received messages for debugging
            print(f"\nReceived {len(collector.messages)} messages in {TEST_TIMEOUT_SECONDS}s")
            for i, msg in enumerate(collector.messages[:5], 1):
                print(f"  [{i}] Position: {msg.latitude:.6f}°, {msg.longitude:.6f}° @ {msg.altitude:.1f}m")
            
            # Check for errors during deserialization
            assert not collector.errors, f"Message deserialization errors: {collector.errors}"
            
            # Assert we got enough messages
            assert success, (
                f"Timeout: Only received {len(collector.messages)} messages, "
                f"expected {MIN_MESSAGES_REQUIRED} within {TEST_TIMEOUT_SECONDS}s. "
                f"Is edgefirst-navsat service running?"
            )
            
        finally:
            subscriber.undeclare()
    
    def test_message_format_validity(self, zenoh_session):
        """Test that received messages have valid NavSatFix format."""
        collector = NavSatCollector()
        subscriber = zenoh_session.declare_subscriber(ZENOH_TOPIC, collector.callback)
        
        try:
            success = collector.wait_for_messages(MIN_MESSAGES_REQUIRED, TEST_TIMEOUT_SECONDS)
            assert success, "Failed to collect messages for validation"
            
            for msg in collector.messages:
                # Validate latitude
                assert -90.0 <= msg.latitude <= 90.0, (
                    f"Invalid latitude: {msg.latitude}"
                )
                
                # Validate longitude
                assert -180.0 <= msg.longitude <= 180.0, (
                    f"Invalid longitude: {msg.longitude}"
                )
                
                # Validate altitude is reasonable
                assert abs(msg.altitude) < 10000.0, (
                    f"Unreasonable altitude: {msg.altitude}m (> 10km)"
                )
                
                # Validate status fields exist
                assert hasattr(msg, 'status'), "Message missing status field"
                assert hasattr(msg.status, 'status'), "Status missing status value"
                assert hasattr(msg.status, 'service'), "Status missing service value"
                
        finally:
            subscriber.undeclare()
    
    def test_gps_fix_quality(self, zenoh_session):
        """Test that GPS receiver provides valid fix status."""
        collector = NavSatCollector()
        subscriber = zenoh_session.declare_subscriber(ZENOH_TOPIC, collector.callback)

        try:
            success = collector.wait_for_messages(MIN_MESSAGES_REQUIRED, TEST_TIMEOUT_SECONDS)
            assert success, "Failed to collect messages for fix quality check"

            # Check fix status on received messages
            # STATUS_FIX (0) or better indicates valid GPS fix
            fix_statuses = [msg.status.status for msg in collector.messages]
            valid_fixes = [s for s in fix_statuses if s >= STATUS_FIX]

            print(f"\nFix status distribution:")
            for status in sorted(set(fix_statuses)):
                count = fix_statuses.count(status)
                pct = (count / len(fix_statuses)) * 100
                fix_type = {
                    -1: "No Fix",
                    0: "GPS Fix",
                    1: "SBAS Fix (WAAS/EGNOS)",
                    2: "GBAS Fix (RTK)"
                }.get(status, f"Unknown ({status})")
                print(f"  Status {status} ({fix_type}): {count} messages ({pct:.1f}%)")

            # Require at least 50% of messages have valid fix (STATUS_FIX or better)
            valid_fix_ratio = len(valid_fixes) / len(fix_statuses)
            assert valid_fix_ratio >= 0.5, (
                f"Insufficient GPS fix quality: only {valid_fix_ratio*100:.1f}% "
                f"of messages have valid fix (status >= {STATUS_FIX}). "
                f"Check GPS antenna placement and sky view."
            )

        finally:
            subscriber.undeclare()
    
    def test_gps_position_reported(self, zenoh_session):
        """Test that GPS position is being reported and record test location."""
        collector = NavSatCollector()
        subscriber = zenoh_session.declare_subscriber(ZENOH_TOPIC, collector.callback)
        
        try:
            success = collector.wait_for_messages(MIN_MESSAGES_REQUIRED, TEST_TIMEOUT_SECONDS)
            assert success, "Failed to collect messages for position check"
            
            # Calculate average position
            latitudes = [msg.latitude for msg in collector.messages]
            longitudes = [msg.longitude for msg in collector.messages]
            altitudes = [msg.altitude for msg in collector.messages]
            
            avg_lat = sum(latitudes) / len(latitudes)
            avg_lon = sum(longitudes) / len(longitudes)
            avg_alt = sum(altitudes) / len(altitudes)
            
            # Print metrics
            print(f"\n=== GPS Position Metrics ===")
            print(f"Messages analyzed: {len(collector.messages)}")
            print(f"Position range:")
            print(f"  Latitude:  {min(latitudes):.6f}° to {max(latitudes):.6f}°")
            print(f"  Longitude: {min(longitudes):.6f}° to {max(longitudes):.6f}°")
            print(f"  Altitude:  {min(altitudes):.1f}m to {max(altitudes):.1f}m")
            print(f"\nTest location (average):")
            print(f"  {avg_lat:.6f}°, {avg_lon:.6f}° @ {avg_alt:.1f}m")
            print("=" * 30)
            
            # Verify position is not default/invalid (0, 0)
            assert not (abs(avg_lat) < 0.001 and abs(avg_lon) < 0.001), (
                "GPS position appears to be default (0, 0). "
                "GPS may not have valid fix."
            )
            
        finally:
            subscriber.undeclare()
    
    def test_message_timestamp_present(self, zenoh_session):
        """Test that messages include valid timestamps."""
        collector = NavSatCollector()
        subscriber = zenoh_session.declare_subscriber(ZENOH_TOPIC, collector.callback)
        
        try:
            success = collector.wait_for_messages(MIN_MESSAGES_REQUIRED, TEST_TIMEOUT_SECONDS)
            assert success, "Failed to collect messages for timestamp check"
            
            for msg in collector.messages:
                # Check header exists
                assert hasattr(msg, 'header'), "Message missing header"
                assert hasattr(msg.header, 'stamp'), "Header missing timestamp"
                
                # Check timestamp has non-zero value
                stamp = msg.header.stamp
                total_time = stamp.sec + stamp.nanosec / 1e9
                assert total_time > 0, (
                    f"Invalid timestamp: {stamp.sec}s {stamp.nanosec}ns"
                )
                
        finally:
            subscriber.undeclare()


if __name__ == "__main__":
    # Allow running with: python3 tests/test_zenoh_integration.py
    pytest.main([__file__, "-v", "--junit-xml=test-results.xml"])

