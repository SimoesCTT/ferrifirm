from pathlib import Path

rom = Path("ferrifirm.bin").read_bytes()

assert len(rom) == 1048576, (
    f"Incorrect ROM size: {len(rom)}"
)

# Reset vector at physical 0xFFFFFFF0,
# corresponding to ROM offset 0xFFFF0.
reset = rom[0xFFFF0:0x100000]

assert reset[:9] == bytes.fromhex(
    "fa fc b0 52 e6 e9 e9 07 00"
), f"Incorrect reset vector: {reset.hex(' ')}"

# Boot stub begins at physical 0xFFFF0000.
stub = rom[0xF0000:0xF1000]

assert stub[:2] == bytes.fromhex("fa fc"), (
    "Boot stub missing or incorrect!"
)

assert bytes.fromhex("b0 53 e6 e9") in stub, (
    "Boot stub S marker missing!"
)

# Long-mode entry begins at physical 0xFFF00000.
firmware = rom[:0x1000]

assert bytes.fromhex("b0 50 ee") in firmware, (
    "Long-mode P marker missing!"
)

print("ROM size:", len(rom))
print("Reset vector:", reset.hex(" "))
print("Boot stub: verified")
print("Long-mode entry marker: verified")
print()
print("SUCCESS: FerriFirm ROM layout verified.")
