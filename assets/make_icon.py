"""Official Grok squircle mark → icon.png + icon.rgba (256x256)."""
from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

SIZE = 256
OUT = Path(__file__).resolve().parent


def round_rect(draw: ImageDraw.ImageDraw, box, radius: int, fill) -> None:
    draw.rounded_rectangle(box, radius=radius, fill=fill)


def main() -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    shadow = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    pad = 18
    box = (pad, pad + 6, SIZE - pad, SIZE - pad + 6)
    round_rect(sd, box, 58, (0, 0, 0, 70))
    shadow = shadow.filter(ImageFilter.GaussianBlur(8))
    img = Image.alpha_composite(img, shadow)
    draw = ImageDraw.Draw(img)
    face = (pad, pad, SIZE - pad, SIZE - pad)
    round_rect(draw, face, 58, (244, 244, 246, 255))
    eye_w, eye_h = 38, 58
    cy = SIZE // 2 + 2
    for cx in (SIZE // 2 - 36, SIZE // 2 + 36):
        draw.rounded_rectangle(
            (cx - eye_w // 2, cy - eye_h // 2, cx + eye_w // 2, cy + eye_h // 2),
            radius=19,
            fill=(18, 18, 20, 255),
        )
    img.save(OUT / "icon.png")
    (OUT / "icon.rgba").write_bytes(img.tobytes())
    print("wrote", OUT / "icon.png", "and icon.rgba", len(img.tobytes()))


if __name__ == "__main__":
    main()
