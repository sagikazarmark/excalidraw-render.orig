import ctypes, sys
lib = ctypes.CDLL(sys.argv[1])
lib.FPDF_InitLibrary()
lib.FPDF_LoadDocument.restype = ctypes.c_void_p
lib.FPDF_LoadPage.restype = ctypes.c_void_p
lib.FPDF_LoadPage.argtypes = [ctypes.c_void_p, ctypes.c_int]
lib.FPDFText_LoadPage.restype = ctypes.c_void_p
lib.FPDFText_LoadPage.argtypes = [ctypes.c_void_p]
lib.FPDFText_CountChars.argtypes = [ctypes.c_void_p]
lib.FPDFText_GetText.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
for path in sys.argv[2:]:
    doc = lib.FPDF_LoadDocument(path.encode(), None)
    page = lib.FPDF_LoadPage(doc, 0)
    text = lib.FPDFText_LoadPage(page)
    n = lib.FPDFText_CountChars(text)
    buf = ctypes.create_string_buffer((n + 1) * 2)
    lib.FPDFText_GetText(text, 0, n, buf)
    print(path.split("/")[-1] + ": " + repr(buf.raw[: n * 2].decode("utf-16-le")))
