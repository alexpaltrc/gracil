# 0062 — Compressing documents (a plan)

**Status:** Proposed (2026-10-07), not built. Asked for by the product owner as a plan for the
future: "Comprimir documentos (PDF, Word, Excel, etc.)". Nothing here is decided until the
product owner accepts it; the measurements in step 0 may change the numbers.

**Context.** Documents are often far heavier than their words: a PDF from a scanner or a phone
holds full-size photos of each page, and a Word, PowerPoint or Excel file holds every picture
pasted into it, usually as it came from the camera. Almost all of that weight is pictures, and
Gracil already makes pictures smaller without changing how they look (ADR 0034). The rest
(text, tables, fonts, formulas, layout) is small and must never change.

Gracil's rules apply as they are: nothing leaves the computer; the person is never asked about
quality or size; the picture size is never reduced (a 4,000-pixel photo stays 4,000 pixels); a
result is kept only when clearly smaller (`WORTH_IT`, at most 80% of the original), otherwise the
file gets the quiet **Already compressed** signal; and what Gracil can't do safely is left exactly
as it is, with a quiet reason, never an error dialog.

**Proposal.**

1. **A third kind of file.** `gracil_core::media::Kind` gains `Document`, known by extension like
   images (ADR 0034): `pdf`, `docx`, `docm`, `xlsx`, `xlsm`, `pptx`, `pptm`, `odt`, `ods`, `odp`.
   Documents join the same list, the same dial, the same **Replace** / **Save copy** / **Always**
   and the same File Explorer menu and file picker; a copy is named like any other
   (`Gracil-Report.pdf`). They compress beside the video, like images, and their pictures are
   counted against the same memory and processor share (`pace.rs`, ADR 0040). There is no
   "getting ready" step.

2. **Word, Excel and PowerPoint (`.docx`, `.xlsx`, `.pptx` and their macro forms), and
   OpenDocument (`.odt`, `.ods`, `.odp`).** These are ZIP folders. Gracil rewrites the folder with
   every part kept byte for byte, except the pictures in its media folders (`word/media`,
   `xl/media`, `ppt/media`, `Pictures`):
   - A JPEG goes through jpegli exactly as a photo does (ADR 0034); a PNG through the PNG path
     (lossless, or a palette when it scores as high). **A picture keeps its format and its name**,
     so the document's own references to it stay valid and nothing else in it is touched. A
     picture that would not get smaller stays as it was.
   - The folder itself is written again with the strongest standard compression (Deflate at its
     highest level; the `mimetype` entry of an OpenDocument file stays first and uncompressed).
   - Left exactly as it is: a file with a digital signature (`_xmlsignatures/`, or
     `META-INF/documentsignatures.xml`), which any change would break; a password-protected file
     (it is not a ZIP folder at all); EMF/WMF drawings; videos and sounds inside a presentation
     (other programs may not play what Gracil would turn them into; a later decision).
   - Crates: `zip` and `flate2` (both MIT/Apache), all inside Gracil, no new program.

3. **PDF.** Two steps, each checked:
   - **Without losing anything:** the file is rewritten with its objects packed together and
     every stream compressed at the highest standard level, unused objects and the history of
     earlier saves removed, and identical pictures and fonts kept once. Pages, text, links,
     bookmarks, forms, tags for screen readers and metadata stay as they are.
   - **Pictures:** each photo inside (DCT-encoded, or a Flate-encoded image that is a photo and has
     no transparency) is compressed with jpegli at the "looks the same" standard of ADR 0034,
     scored with SSIMULACRA2 against the original, at the same pixel size. Its transparency mask,
     if any, is a separate image and is kept.
   - Left exactly as it is: a signed PDF (a `/Sig` field with a `/ByteRange`), an encrypted or
     password-protected PDF, and a PDF/A file beyond the lossless step (an archive copy must stay
     conformant).
   - Engine: qpdf (Apache-2.0, compatible with Gracil's GPL-3.0), shipped beside `gracil.exe` like
     FFmpeg and jpegli, pinned and with its license in the zip, for the lossless rewrite and its
     check; the pictures read and replaced inside Gracil with `lopdf` (MIT). Step 0 confirms the
     pair, or that `lopdf` alone is enough, which would ship no new program.
   - Not used: Ghostscript (AGPL, and it rebuilds the whole file, which can change text, drop
     tags and forms, and make some files bigger); lossy JBIG2 for black-and-white scans (it can
     swap one letter or digit for another that looks alike).

4. **Checked before it is kept,** as a video is checked before it replaces the original. A ZIP
   document opens again, holds the same parts under the same names, and every part but the
   replaced pictures is identical. A PDF passes `qpdf --check` and has the same number of pages,
   and every replaced picture has the same pixel size. Anything else: the original stays, with
   the quiet "couldn't compress" reason.

5. **Order of work.**
   - **Step 0, measuring.** A set of real documents chosen by the product owner, read on their
     computer only: how much each kind shrinks, how long it takes, and that Word, Excel,
     PowerPoint, LibreOffice and two PDF readers open every result unchanged.
   - **Step 1:** Word, Excel, PowerPoint and OpenDocument (no new program, reuses the image path).
   - **Step 2:** PDF without losing anything.
   - **Step 3:** pictures inside PDFs.

**Not planned.** Older Office files (`.doc`, `.xls`, `.ppt`) and plain text, CSV and RTF files
are left as they are: there is either nothing heavy in them or no safe way to change them. Making
text in scanned PDFs searchable (OCR) is a different feature, not compression.

**Consequences, if accepted.**

- People can drop a folder of mixed files and every video, photo and document in it gets
  lighter, with the same three choices and nothing new to learn.
- The zip may grow by qpdf's size (a few MB) from step 2, and `licenses.ps1` gains its license.
- The spec gains document rules beside §77.1 (images), and the guide, the FAQ and the changelog
  say what documents Gracil compresses and which it leaves alone.
