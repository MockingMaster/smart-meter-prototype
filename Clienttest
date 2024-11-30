import unittest
import socket 
from unittest.mock import Mock, patch
from ClientSide import (
    generate_meter_reading,
    send_reading_to_server,
    authenticate,
)
import struct

# Helper function for creating headers
def create_header(length):
    """Create a 2-byte header for the given message length."""
    return struct.pack('>H', length)

class TestSmartMeter(unittest.TestCase):
    def test_generate_meter_reading(self):
        """Test that generated meter readings are within the expected range."""
        for _ in range(100):
            reading = generate_meter_reading()
            self.assertGreaterEqual(reading, 0.5)
            self.assertLessEqual(reading, 2.5)

    @patch("ClientSide.socket.socket")
    def test_send_reading_to_server_success(self, mock_socket):
        """Test sending a meter reading to the server successfully."""
        mock_socket_instance = mock_socket.return_value
        mock_socket_instance.send.return_value = len(b"test")
        # Simulate server response: header indicating 3 bytes followed by "ACK"
        mock_socket_instance.recv.side_effect = [create_header(3), b"ACK"]
        
        result = send_reading_to_server(mock_socket_instance, {"type": "MeterReading", "reading": 5.0})
        self.assertEqual(result, "")
        mock_socket_instance.send.assert_called()

    @patch("ClientSide.socket.socket")
    def test_send_reading_to_server_failure(self, mock_socket):
        """Test failure in sending a meter reading to the server."""
        mock_socket_instance = mock_socket.return_value
        # Simulate a socket error when sending
        mock_socket_instance.send.side_effect = socket.error("Socket error")
        #https://docs.python.org/3/library/unittest.mock.html#unittest.mock.Mock.side_effect
        result = send_reading_to_server(mock_socket_instance, {"type": "MeterReading", "reading": 5.0})
        self.assertIn("Error", result)

    @patch("ClientSide.socket.socket")
    def test_authenticate_success(self, mock_socket):
        """Test successful authentication with the server."""
        mock_socket_instance = mock_socket.return_value
        # Simulate server response: header indicating 24 bytes followed by "Authentication successful"
        mock_socket_instance.recv.side_effect = [create_header(24), b"Authentication successful"]

        result = authenticate(mock_socket_instance, 12345)
        self.assertTrue(result)

    @patch("ClientSide.socket.socket")
    def test_authenticate_failure(self, mock_socket):
        """Test failed authentication with the server."""
        mock_socket_instance = mock_socket.return_value
        # Simulate server response: header indicating 18 bytes followed by "Authentication failed"
        mock_socket_instance.recv.side_effect = [create_header(18), b"Authentication failed"]

        result = authenticate(mock_socket_instance, 12345)
        self.assertFalse(result)

    @patch("ClientSide.socket.socket")
    def test_authenticate_timeout(self, mock_socket):
        """Test timeout during authentication."""
        mock_socket_instance = mock_socket.return_value
        # Simulate a socket timeout
        mock_socket_instance.recv.side_effect = socket.timeout

        result = authenticate(mock_socket_instance, 12345)
        self.assertFalse(result)

if __name__ == "__main__":
    unittest.main()
