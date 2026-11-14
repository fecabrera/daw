"""Guard the native loader invariant even on workers without macOS dyld."""
import struct
import unittest
from check_auv2_exports import validate_macho_linkedit


def macho(strings=64, length=8):
    header = struct.pack('<8I', 0xFEEDFACF, 0x0100000C, 0, 6, 1, 24, 0, 0)
    table = struct.pack('<6I', 2, 24, 0, 0, strings, length)
    return header + table + bytes(80)


class MachoAlignment(unittest.TestCase):
    def test_aligned_linker_output_is_accepted(self):
        validate_macho_linkedit(macho())

    def test_stripped_output_with_four_byte_string_alignment_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'LINKEDIT'):
            validate_macho_linkedit(macho(strings=68))

    def test_truncated_headers_tables_and_ranges_are_rejected(self):
        for binary in (b'bad', macho()[:40], macho(strings=64, length=1000)):
            with self.subTest(binary=binary[:32]), self.assertRaises(ValueError):
                validate_macho_linkedit(binary)
