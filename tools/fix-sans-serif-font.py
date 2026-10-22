#!/usr/bin/env python3
"""Fix the generic sans-serif alias so it stops resolving to a symbol font.

The problem
-----------

`fc-match sans-serif` can return Sun-ExtA:

    $ fc-match sans-serif
    Sun-ExtA.ttf: "Sun-ExtA" "Regular"

Sun-ExtA is a symbol font — it holds the Sun font's extended symbol set, not
text. It claims support for more than two hundred languages, so under a Chinese
locale it wins the language match and every GTK and WebKit application that asks
for a sans-serif font is handed a symbol font. The text renders as the wrong
glyphs, in Latin as well as Chinese, because the fallback chain is decided by
the same match.

This is a system configuration problem rather than an application problem, and
it affects everything on the desktop, not only this player.

The fix
-------

A configuration file that gives the alias a font with both Latin and Chinese
glyphs, using the same strong-prepend trick that is already used for
`monospace` in most setups:

    ~/.config/fontconfig/conf.d/62-sans-serif.conf

Source Han Sans SC is the default here because its Latin is Source Sans, so
the two scripts match each other and text does not change shape between them.
Any font with both scripts works; the script takes the name as an argument.

What this script does
---------------------

Writes the configuration file, rebuilds the font cache, and reports what the
alias resolves to before and after. It changes nothing outside
`~/.config/fontconfig/conf.d/`, and it can be undone by deleting the file it
writes and running `fc-cache -f`.
"""

import argparse
import os
import pathlib
import subprocess
import sys

CONF_DIR = pathlib.Path.home() / ".config" / "fontconfig" / "conf.d"
CONF_NAME = "62-sans-serif.conf"

TEMPLATE = """<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<!--
Force the generic sans-serif alias to a font that has both Latin and Chinese
glyphs.

Without this, `fc-match sans-serif` resolves to Sun-ExtA: a symbol font that
claims support for 200+ languages, so it wins the language match under a Chinese
locale and every GTK and WebKit application gets a symbol font where it asked
for a text font. The same trap is fixed for monospace elsewhere; sans-serif was
left alone.

{font} carries Latin glyphs that match its Chinese ones, so text does not change
shape between the two scripts.

A strong prepend beats the language bonus that gives Sun-ExtA its place.
-->
<fontconfig>
  <match target="pattern">
    <test name="family" qual="any">
      <string>sans-serif</string>
    </test>
    <edit name="family" mode="prepend" binding="strong">
      <string>{font}</string>
    </edit>
  </match>
</fontconfig>
"""


def match(pattern: str) -> str:
    """What fontconfig resolves `pattern` to, or a message if it cannot say."""
    try:
        result = subprocess.run(
            ["fc-match", pattern],
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired) as err:
        return f"could not run fc-match: {err}"
    return result.stdout.strip() or "no match"


def has_font(name: str) -> bool:
    """Whether fontconfig knows a family by that name."""
    try:
        result = subprocess.run(
            ["fc-list", ":family=" + name, "family"],
            capture_output=True,
            text=True,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return bool(result.stdout.strip())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument(
        "--font",
        default="Source Han Sans SC",
        help="the family to use for sans-serif (default: Source Han Sans SC)",
    )
    parser.add_argument(
        "--remove",
        action="store_true",
        help="remove the configuration file and restore the font cache",
    )
    args = parser.parse_args()

    target = CONF_DIR / CONF_NAME

    if args.remove:
        if target.exists():
            target.unlink()
            print(f"removed {target}")
        else:
            print(f"nothing to remove at {target}")
        subprocess.run(["fc-cache", "-f"], capture_output=True)
        print(f"sans-serif now resolves to: {match('sans-serif')}")
        return 0

    print(f"before:  sans-serif -> {match('sans-serif')}")

    if not has_font(args.font):
        print(
            f"error: fontconfig does not know a family called {args.font!r}.",
            file=sys.stderr,
        )
        print("List the families with: fc-list : family | sort -u", file=sys.stderr)
        return 1

    CONF_DIR.mkdir(parents=True, exist_ok=True)
    target.write_text(TEMPLATE.format(font=args.font))
    print(f"wrote {target}")

    subprocess.run(["fc-cache", "-f"], capture_output=True)

    print(f"after:   sans-serif -> {match('sans-serif')}")
    print(f"         sans-serif:lang=zh -> {match('sans-serif:lang=zh')}")
    print(f"         sans-serif:lang=en -> {match('sans-serif:lang=en')}")
    print()
    print("Undo with: python3 tools/fix-sans-serif-font.py --remove")
    return 0


if __name__ == "__main__":
    sys.exit(main())
