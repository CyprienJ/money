#!/usr/bin/env python3
"""Anonymize a bank statement PDF while keeping its layout.

Each search string is replaced in place: the original characters are removed from the
PDF (true redaction, not a black box over the text) and the replacement text is written
at the same position, so that a positional parser still sees the same structure.

Replacements file, UTF-8, one rule per line:

    # comment
    JORANT CYPRIEN => DUPONT JEAN
    FR76 1451 8292 6700 0867 1514 096 => FR76 0000 0000 0000 0000 0000 000

Matching ignores case. Write every form a value takes in the statement (with and without
spaces, upper and lower case words in a different order...). After saving, the script
re-reads the output and fails if a rule matched nothing or if any search string is still
found, spaces ignored.

Usage:
    pip install pymupdf
    python tools/anonymize_pdf.py input.pdf output.pdf replacements.txt
"""

import argparse
import sys

import pymupdf

MIN_FONT_SIZE = 4.0


def read_rules(path):
    rules = []
    with open(path, encoding="utf-8") as file:
        for number, line in enumerate(file, start=1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "=>" not in line:
                sys.exit(f"{path}:{number}: expected 'original => replacement'")
            original, replacement = (part.strip() for part in line.split("=>", 1))
            if not original:
                sys.exit(f"{path}:{number}: empty original text")
            rules.append((original, replacement))
    # Longest first, so that a full name is replaced before a part of it.
    return sorted(rules, key=lambda rule: len(rule[0]), reverse=True)


def fitting_font_size(text, rect):
    """Largest font size, up to the line height, at which `text` fits in `rect`."""
    size = rect.height * 0.8
    while size > MIN_FONT_SIZE and pymupdf.get_text_length(text, "helv", size) > rect.width:
        size -= 0.25
    return max(size, MIN_FONT_SIZE)


def middle_band(rect):
    """Thin horizontal band in the middle of a match. Every character overlapping a
    redaction is removed, and match boxes overlap the lines above and below."""
    middle = (rect.y0 + rect.y1) / 2
    return pymupdf.Rect(rect.x0 + 0.5, middle - 0.5, rect.x1 - 0.5, middle + 0.5)


def word_matches(words, original):
    """Boxes of `original` matched word by word, so that any spacing between the words of
    the PDF is accepted (some statements put two spaces inside account numbers)."""
    tokens = original.casefold().split()
    matches = []
    for start in range(len(words) - len(tokens) + 1):
        candidates = words[start : start + len(tokens)]
        same_line = all(word[5:7] == candidates[0][5:7] for word in candidates)
        if same_line and [word[4].casefold() for word in candidates] == tokens:
            box = pymupdf.Rect(candidates[0][:4])
            for word in candidates[1:]:
                box |= pymupdf.Rect(word[:4])
            matches.append(box)
    return matches


def redact(document, rules):
    counts = {original: 0 for original, _ in rules}
    for page in document:
        words = page.get_text("words")
        taken = []
        for original, replacement in rules:
            for rect in page.search_for(original) + word_matches(words, original):
                band = middle_band(rect)
                # Skip a second match of the same text, or a shorter rule matching inside
                # a longer one already redacted.
                if any(band.intersects(other) for other, _, _ in taken):
                    continue
                taken.append((band, rect, replacement))
                counts[original] += 1
                page.add_redact_annot(band, fill=False, cross_out=False)
        # Remove only the text: keep table lines and logos.
        page.apply_redactions(
            images=pymupdf.PDF_REDACT_IMAGE_NONE,
            graphics=pymupdf.PDF_REDACT_LINE_ART_NONE,
        )
        for _, rect, replacement in taken:
            size = fitting_font_size(replacement, rect)
            baseline = pymupdf.Point(rect.x0, rect.y1 - rect.height * 0.22)
            page.insert_text(baseline, replacement, fontname="helv", fontsize=size)
    return counts


def strip_extras(document):
    document.set_metadata({})
    document.del_xml_metadata()
    document.set_toc([])
    for name in document.embfile_names():
        document.embfile_del(name)
    for page in document:
        for link in page.get_links():
            page.delete_link(link)
        for annot in list(page.annots()):
            page.delete_annot(annot)


def leftovers(path, rules):
    """Search strings still present in the saved file, spaces and case ignored."""
    with pymupdf.open(path) as document:
        text = "".join(page.get_text() for page in document)
    compact = "".join(text.split()).casefold()
    return [original for original, _ in rules if "".join(original.split()).casefold() in compact]


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("input")
    parser.add_argument("output")
    parser.add_argument("replacements")
    arguments = parser.parse_args()

    rules = read_rules(arguments.replacements)
    with pymupdf.open(arguments.input) as document:
        counts = redact(document, rules)
        strip_extras(document)
        document.save(arguments.output, garbage=4, deflate=True, clean=True)

    for original, count in counts.items():
        print(f"{count:3} × {original}")
    failed = False
    unmatched = [original for original, count in counts.items() if count == 0]
    if unmatched:
        failed = True
        print("\nNOT FOUND (copy the exact form from the PDF text, see pdftotext -raw):")
        for original in unmatched:
            print(f"  {original}")
    remaining = leftovers(arguments.output, rules)
    if remaining:
        failed = True
        print("\nSTILL PRESENT in the output:")
        for original in remaining:
            print(f"  {original}")
    if failed:
        sys.exit(1)
    print(f"\nOK: {arguments.output}")


if __name__ == "__main__":
    main()
