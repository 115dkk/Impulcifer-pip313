"""PID-scoped Win32 screenshots and native confirmation, without Pillow."""
import ctypes as C
from ctypes import wintypes as W
import struct
import zlib


def api():
    user = C.WinDLL("user32", use_last_error=True)
    gdi = C.WinDLL("gdi32", use_last_error=True)
    callback = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
    signatures = (
        (user.EnumWindows, [callback, W.LPARAM], W.BOOL),
        (user.EnumChildWindows, [W.HWND, callback, W.LPARAM], W.BOOL),
        (user.GetWindowThreadProcessId, [W.HWND, C.POINTER(W.DWORD)], W.DWORD),
        (user.IsWindowVisible, [W.HWND], W.BOOL),
        (user.GetWindowRect, [W.HWND, C.POINTER(W.RECT)], W.BOOL),
        (user.GetWindowTextW, [W.HWND, W.LPWSTR, C.c_int], C.c_int),
        (user.GetClassNameW, [W.HWND, W.LPWSTR, C.c_int], C.c_int),
        (user.GetDlgCtrlID, [W.HWND], C.c_int),
        (user.PostMessageW, [W.HWND, W.UINT, W.WPARAM, W.LPARAM], W.BOOL),
        (user.GetWindowDC, [W.HWND], W.HDC),
        (user.ReleaseDC, [W.HWND, W.HDC], C.c_int),
        (user.PrintWindow, [W.HWND, W.HDC, W.UINT], W.BOOL),
        (gdi.CreateCompatibleDC, [W.HDC], W.HDC),
        (gdi.CreateCompatibleBitmap, [W.HDC, C.c_int, C.c_int], W.HBITMAP),
        (gdi.SelectObject, [W.HDC, W.HGDIOBJ], W.HGDIOBJ),
        (gdi.GetDIBits, [W.HDC, W.HBITMAP, W.UINT, W.UINT, C.c_void_p, C.c_void_p, W.UINT], C.c_int),
        (gdi.DeleteObject, [W.HGDIOBJ], W.BOOL),
        (gdi.DeleteDC, [W.HDC], W.BOOL),
    )
    for function, args, result in signatures:
        function.argtypes = args
        function.restype = result
    return user, gdi, callback


def windows(pid):
    user, _, callback = api()
    result = []

    @callback
    def visit(hwnd, _):
        owner = W.DWORD()
        user.GetWindowThreadProcessId(hwnd, C.byref(owner))
        if owner.value == pid and user.IsWindowVisible(hwnd):
            title = C.create_unicode_buffer(512)
            kind = C.create_unicode_buffer(128)
            user.GetWindowTextW(hwnd, title, len(title))
            user.GetClassNameW(hwnd, kind, len(kind))
            result.append((hwnd, title.value, kind.value))
        return True

    if not user.EnumWindows(visit, 0):
        raise C.WinError(C.get_last_error())
    return result


def capture(pid, path):
    user, gdi, _ = api()
    candidates = [h for h, title, _ in windows(pid) if title == "Impulcifer"]
    assert len(candidates) == 1, f"main HWND missing/ambiguous for PID {pid}: {windows(pid)}"
    hwnd = candidates[0]
    rect = W.RECT()
    assert user.GetWindowRect(hwnd, C.byref(rect)), "GetWindowRect failed"
    width, height = rect.right - rect.left, rect.bottom - rect.top
    assert 0 < width < 10000 and 0 < height < 10000, (width, height)
    dc = user.GetWindowDC(hwnd)
    assert dc, "GetWindowDC failed"
    memory = bitmap = previous = None
    try:
        memory = gdi.CreateCompatibleDC(dc)
        assert memory, "CreateCompatibleDC failed"
        bitmap = gdi.CreateCompatibleBitmap(dc, width, height)
        assert bitmap, "CreateCompatibleBitmap failed"
        previous = gdi.SelectObject(memory, bitmap)
        assert previous, "SelectObject failed"
        # PW_RENDERFULLCONTENT includes the accelerated WebView2 surface.
        assert user.PrintWindow(hwnd, memory, 2), "PrintWindow failed"
        gdi.SelectObject(memory, previous)
        previous = None
        info = C.create_string_buffer(struct.pack("<IiiHHIIiiII", 40, width, -height, 1, 32, 0,
                                                width * height * 4, 0, 0, 0, 0))
        pixels = C.create_string_buffer(width * height * 4)
        assert gdi.GetDIBits(dc, bitmap, 0, height, pixels, info, 0) == height, "GetDIBits failed"
        raw = pixels.raw
        rgb = bytearray(width * height * 3)
        rgb[0::3], rgb[1::3], rgb[2::3] = raw[2::4], raw[1::4], raw[0::4]
        assert len(set(rgb)) > 8, "blank PrintWindow capture"
        scanlines = b"".join(b"\0" + rgb[y * width * 3:(y + 1) * width * 3] for y in range(height))

        def chunk(kind, data):
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

        path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                         + chunk(b"IDAT", zlib.compress(scanlines)) + chunk(b"IEND", b""))
        return {"hwnd": hwnd, "width": width, "height": height, "path": str(path)}
    finally:
        if previous:
            gdi.SelectObject(memory, previous)
        if bitmap:
            gdi.DeleteObject(bitmap)
        if memory:
            gdi.DeleteDC(memory)
        user.ReleaseDC(hwnd, dc)


def accept_confirmation(pid):
    """Click only IDOK/IDYES in a native dialog owned by this app's PID."""
    user, _, callback = api()
    accepted = []
    for hwnd, title, kind in windows(pid):
        if kind != "#32770":
            continue
        controls = []

        @callback
        def child(control, _):
            text = C.create_unicode_buffer(4096)
            user.GetWindowTextW(control, text, len(text))
            controls.append((control, user.GetDlgCtrlID(control), text.value))
            return True

        user.EnumChildWindows(hwnd, child, 0)
        text = " ".join(item[2] for item in controls)
        # Never accept an unrelated error/permission dialog.
        if "headphone" not in text.lower():
            continue
        button = next((control for control, identifier, _ in controls if identifier in (1, 6)), None)
        if button:
            assert user.PostMessageW(button, 0x00F5, 0, 0), "BM_CLICK failed"
            accepted.append({"title": title, "text": text, "hwnd": hwnd})
    return accepted
