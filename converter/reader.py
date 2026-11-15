"""WOLF v3 二进制游标。

语义与 crates/wolf-format/src/bytes.rs 一一对应，任何偏移改动都必须两边同时改，
否则对账会失败。字符串规则：t_str 是 u32 小端字节长度（含结尾 NUL）+ 载荷。
"""
from __future__ import annotations

import struct


class WolfEof(Exception):
    """读取越界。带上偏移，方便定位损坏的输入文件。"""


class Reader:
    __slots__ = ("buf", "pos", "warnings")

    def __init__(self, buf: bytes, pos: int = 0) -> None:
        self.buf = buf
        self.pos = pos
        self.warnings: list = []

    # -- 位置 ---------------------------------------------------------------
    def remaining(self) -> int:
        return len(self.buf) - self.pos

    def at_end(self) -> bool:
        return self.pos == len(self.buf)

    def take(self, n: int) -> bytes:
        if n < 0 or self.remaining() < n:
            raise WolfEof(f"eof at {self.pos}: want {n}, have {self.remaining()}")
        out = self.buf[self.pos : self.pos + n]
        self.pos += n
        return out

    def skip(self, n: int) -> None:
        self.take(n)

    # -- 整数（全部小端，无填充）------------------------------------------
    def u8(self) -> int:
        return self.take(1)[0]

    def u16(self) -> int:
        return struct.unpack("<H", self.take(2))[0]

    def u32(self) -> int:
        return struct.unpack("<I", self.take(4))[0]

    def i32(self) -> int:
        return struct.unpack("<i", self.take(4))[0]

    def u64(self) -> int:
        return struct.unpack("<Q", self.take(8))[0]

    def u32_array(self, n: int) -> list:
        if n > self.remaining() // 4:
            raise WolfEof(f"eof at {self.pos}: u32_array({n}) needs {n * 4}, have {self.remaining()}")
        return list(struct.unpack("<%dI" % n, self.take(4 * n))) if n else []

    def i32_array(self, n: int) -> list:
        if n > self.remaining() // 4:
            raise WolfEof(f"eof at {self.pos}: i32_array({n}) needs {n * 4}, have {self.remaining()}")
        return list(struct.unpack("<%di" % n, self.take(4 * n))) if n else []

    def t_u32_array(self) -> list:
        return self.u32_array(self.u32())

    def t_i32_array(self) -> list:
        n = self.i32()
        return [] if n <= 0 else self.i32_array(n)

    # -- 字符串 -------------------------------------------------------------
    def t_str_bytes(self) -> bytes:
        n = self.u32()
        if n == 0:
            return b""
        raw = self.take(n)
        return raw[:-1] if raw[-1:] == b"\x00" else raw

    def t_str(self) -> str:
        return self.decode(self.t_str_bytes())

    def decode(self, raw: bytes) -> str:
        try:
            return raw.decode("utf-8")
        except UnicodeDecodeError:
            self.warnings.append({"pos": self.pos, "bytes": len(raw)})
            return raw.decode("utf-8", "replace")

    def t_str_array(self) -> list:
        return [self.t_str() for _ in range(self.u32())]
