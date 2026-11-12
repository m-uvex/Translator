//! Symboldaten, erzeugt aus den SVG-Definitionen der Electron-Fassung
//! (`IC` in renderer.js und editor.js). Nicht von Hand bearbeiten;
//! das erzeugende Skript liegt in `.tools/gen_icons.py`.

use crate::icons::Shape;

const SLIDERS: &[Shape] = &[Shape::Path("M21 4h-7M10 4H3M21 12h-9M8 12H3M21 20h-5M12 20H3M14 2v4M8 10v4M16 18v4")];
const SWAP: &[Shape] = &[Shape::Path("M17 4l4 4-4 4M21 8H7M7 20l-4-4 4-4M3 16h14")];
const REFRESH: &[Shape] = &[Shape::Path("M21 12a9 9 0 1 1-3-6.7L21 8M21 3v5h-5")];
const CLIP: &[Shape] = &[Shape::Path("m21.44 11.05-9.19 9.19a6 6 0 0 1-8.49-8.49l8.57-8.57A4 4 0 1 1 18 8.84l-8.59 8.57a2 2 0 0 1-2.83-2.83l8.49-8.48")];
const COPY: &[Shape] = &[Shape::Path("M5 15V6a2 2 0 0 1 2-2h9"), Shape::Rect(9.0, 9.0, 11.0, 11.0, 3.0)];
const STOP: &[Shape] = &[Shape::Path("M7.86 2h8.28L22 7.86v8.28L16.14 22H7.86L2 16.14V7.86z")];
const STAR: &[Shape] = &[Shape::Path("M12 3l2.8 5.7 6.2.9-4.5 4.4 1 6.2-5.5-3-5.5 3 1-6.2L3 9.6l6.2-.9z")];
const X: &[Shape] = &[Shape::Path("M6 6l12 12M18 6L6 18")];
const CHECK: &[Shape] = &[Shape::Path("M5 12l5 5 9-10")];
const EYE: &[Shape] = &[Shape::Path("M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"), Shape::Circle(12.0, 12.0, 3.0)];
const BULB: &[Shape] = &[Shape::Path("M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2.1h5c0-.9.4-1.6 1-2.1A6 6 0 0 0 12 3z")];
const LANGUAGES: &[Shape] = &[Shape::Path("M4 6h9M8.5 4v2M6 6c.8 3 3 5.5 6 7M12 6c-.8 3-3 5.5-6 7M13 20l4-9 4 9M14.5 17h5")];
const DOC: &[Shape] = &[Shape::Path("M6 3h8l4 4v14H6z"), Shape::Path("M14 3v4h4M9 12h6M9 16h6")];
const LIBRARY: &[Shape] = &[Shape::Path("m16 6 4 14M12 6v14M8 8v12M4 4v16")];
const NATURAL: &[Shape] = &[Shape::Path("M3 9c3-4 6-4 9 0s6 4 9 0"), Shape::Path("M3 15c3-4 6-4 9 0s6 4 9 0")];
const LITERAL: &[Shape] = &[Shape::Path("M5 9h14M5 15h14")];
const QMARK: &[Shape] = &[Shape::Path("M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.4-1 1-1 1.7M12 17h.01"), Shape::Circle(12.0, 12.0, 9.0)];
const NB: &[Shape] = &[Shape::Path("M12 9V2M8.5 11 3 8M15.5 11 21 8"), Shape::Circle(12.0, 13.0, 4.0)];
const MALE: &[Shape] = &[Shape::Path("M14 10l6-6M15 4h5v5"), Shape::Circle(10.0, 14.0, 5.0)];
const FEMALE: &[Shape] = &[Shape::Path("M12 14v8M9 18h6"), Shape::Circle(12.0, 9.0, 5.0)];
const HAT: &[Shape] = &[Shape::Path("M6 15V6a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v9M3 15h18M4 15v1a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-1M6 11h12")];
const DASH: &[Shape] = &[Shape::Path("M6 12h12")];
const SMILE: &[Shape] = &[Shape::Path("M8 14c1 1.5 2.3 2 4 2s3-.5 4-2M9 9.5h.01M15 9.5h.01"), Shape::Circle(12.0, 12.0, 9.0)];
const WINMIN: &[Shape] = &[Shape::Path("M5 12h14")];
const WINMAX: &[Shape] = &[Shape::Rect(5.0, 5.0, 14.0, 14.0, 2.0)];
const WINRESTORE: &[Shape] = &[Shape::Path("M5 16V6a1 1 0 0 1 1-1h10"), Shape::Rect(8.0, 8.0, 11.0, 11.0, 2.0)];
const UNDO: &[Shape] = &[Shape::Path("M9 14L4 9l5-5"), Shape::Path("M4 9h10a6 6 0 0 1 0 12h-3")];
const REDO: &[Shape] = &[Shape::Path("M15 14l5-5-5-5"), Shape::Path("M20 9H10a6 6 0 0 0 0 12h3")];
const BOLD: &[Shape] = &[Shape::Path("M7 5h6a3.5 3.5 0 0 1 0 7H7zM7 12h7a3.5 3.5 0 0 1 0 7H7z")];
const ITALIC: &[Shape] = &[Shape::Path("M14 5h-4M14 19h-4M15 5l-6 14")];
const UNDERLINE: &[Shape] = &[Shape::Path("M7 4v7a5 5 0 0 0 10 0V4M5 20h14")];
const STRIKE: &[Shape] = &[Shape::Path("M5 12h14M16 7a4 4 0 0 0-4-2c-2.5 0-4 1.2-4 3 0 4 8 2.5 8 7 0 1.8-1.7 3-4.2 3a4.6 4.6 0 0 1-4.3-2.5")];
const UL: &[Shape] = &[Shape::Path("M9 6h11M9 12h11M9 18h11M4.5 6h.01M4.5 12h.01M4.5 18h.01")];
const OL: &[Shape] = &[Shape::Path("M10 6h10M10 12h10M10 18h10M4 5l1-.5V9M4 14h2l-2 3h2")];
const QUOTE: &[Shape] = &[Shape::Path("M6 8c-1.5 1-2 2.5-2 4v4h5v-5H6.5M15 8c-1.5 1-2 2.5-2 4v4h5v-5h-2.5")];
const ALIGNL: &[Shape] = &[Shape::Path("M4 6h16M4 10h10M4 14h16M4 18h10")];
const ALIGNC: &[Shape] = &[Shape::Path("M4 6h16M7 10h10M4 14h16M7 18h10")];
const ALIGNR: &[Shape] = &[Shape::Path("M4 6h16M10 10h10M4 14h16M10 18h10")];
const ALIGNJ: &[Shape] = &[Shape::Path("M4 6h16M4 10h16M4 14h16M4 18h16")];
const LINK: &[Shape] = &[Shape::Path("M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1")];
const ERASER: &[Shape] = &[Shape::Path("M20 20H9l-5-5a2 2 0 0 1 0-3l8-8a2 2 0 0 1 3 0l5 5a2 2 0 0 1 0 3l-7 7")];
const FILENEW: &[Shape] = &[Shape::Path("M6 3h8l4 4v14H6z"), Shape::Path("M14 3v4h4M12 11v6M9 14h6")];
const FOLDER: &[Shape] = &[Shape::Path("M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z")];
const SAVE: &[Shape] = &[Shape::Path("M12 4v11M7 10l5 5 5-5M5 20h14")];
const SHORTEN: &[Shape] = &[Shape::Path("M12 3v7M8.5 7l3.5 3 3.5-3M12 21v-7M8.5 17l3.5-3 3.5 3M4 12h16")];
const LENGTHEN: &[Shape] = &[Shape::Path("M12 10V3M8.5 6l3.5-3 3.5 3M12 14v7M8.5 18l3.5 3 3.5-3M4 12h16")];
const SUMMARIZE: &[Shape] = &[Shape::Path("M4 6h16M6 10h12M8 14h8M10 18h4")];
const PROOFREAD: &[Shape] = &[Shape::Path("M3 17l3-9 3 9M4.2 14h3.6M13 17l3 3 6-7")];
const SYNONYMS: &[Shape] = &[Shape::Path("M4 8h13l-3-3M20 16H7l3 3")];
const WAND: &[Shape] = &[Shape::Path("m21.64 3.64-1.28-1.28a1.21 1.21 0 0 0-1.72 0L2.36 18.64a1.21 1.21 0 0 0 0 1.72l1.28 1.28a1.2 1.2 0 0 0 1.72 0L21.64 5.36a1.2 1.2 0 0 0 0-1.72"), Shape::Path("m14 7 3 3"), Shape::Path("M5 6v4"), Shape::Path("M19 14v4"), Shape::Path("M10 2v2"), Shape::Path("M7 8H3"), Shape::Path("M21 16h-4"), Shape::Path("M11 3H9")];
const PLUS: &[Shape] = &[Shape::Path("M12 5v14M5 12h14")];
const PENCIL: &[Shape] = &[Shape::Path("M4 20l1-4L16 5l3 3L8 19zM14 7l3 3")];
const CPU: &[Shape] = &[Shape::Path("M9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"), Shape::Rect(6.0, 6.0, 12.0, 12.0, 3.0)];
const CHEV: &[Shape] = &[Shape::Path("M6 9l6 6 6-6")];
const ARROWR: &[Shape] = &[Shape::Path("M5 12h14M13 6l6 6-6 6")];
const UPLOAD: &[Shape] = &[Shape::Path("M12 16V3M7 8l5-5 5 5"), Shape::Path("M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4")];
const DOWNLOAD: &[Shape] = &[Shape::Path("M12 3v13M7 11l5 5 5-5"), Shape::Path("M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4")];
const DOCS: &[Shape] = &[Shape::Rect(1.6, 4.8, 3.4, 14.4, 0.0), Shape::Rect(8.4, 5.6, 7.2, 12.8, 0.0), Shape::Rect(19.0, 4.8, 3.4, 14.4, 0.0)];
const BOLT: &[Shape] = &[Shape::Path("M13 2L4 14h7l-1 8 9-12h-7z")];
const GRADUATION: &[Shape] = &[Shape::Path("M2 9l10-5 10 5-10 5zM6 11.5V16c0 1.5 3 3 6 3s6-1.5 6-3v-4.5")];
const SPARKLES: &[Shape] = &[Shape::Path("M12 3l1.8 4.7 4.7 1.8-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8z"), Shape::Path("M19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z")];
const FEATHER: &[Shape] = &[Shape::Path("M20 4c-8 0-14 5-14 12v4M20 4c0 8-5 12-12 12M6 20l6-6")];
const BRIEFCASE: &[Shape] = &[Shape::Path("M9 7V5a2 2 0 0 1 2-2h2a2 2 0 0 1 2 2v2M3 13h18"), Shape::Rect(3.0, 7.0, 18.0, 13.0, 3.0)];
const BOOK: &[Shape] = &[Shape::Path("M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2zM4 19V5M8 7h7")];
const HEART: &[Shape] = &[Shape::Path("M12 20s-8-5-8-11a4.5 4.5 0 0 1 8-2.7A4.5 4.5 0 0 1 20 9c0 6-8 11-8 11z")];
const COFFEE: &[Shape] = &[Shape::Path("M5 8h11v6a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5zM16 10h2a2 2 0 0 1 0 4h-2M8 3v2M12 3v2")];
const MEGAPHONE: &[Shape] = &[Shape::Path("M3 11v2a1 1 0 0 0 1 1h3l8 4V6L7 10H4a1 1 0 0 0-1 1zM19 9a4 4 0 0 1 0 6")];
const LEAF: &[Shape] = &[Shape::Path("M5 19c0-9 5-14 15-14 0 10-5 15-14 15M5 19l8-8")];
const ROCKET: &[Shape] = &[Shape::Path("M4.5 16.5c-1.5 1.3-2 5-2 5s3.7-.5 5-2c.7-.9.7-2.2-.1-3-.8-.8-2.1-.8-2.9 0z"), Shape::Path("m12 15-3-3a22 22 0 0 1 2-3.95A12.88 12.88 0 0 1 22 2c0 2.72-.78 7.5-6 11a22.35 22.35 0 0 1-4 2z"), Shape::Path("M9 12H4s.55-3.03 2-4c1.62-1.08 5 0 5 0M12 15v5s3.03-.55 4-2c1.08-1.62 0-5 0-5")];
const CROWN: &[Shape] = &[Shape::Path("M3 7l4 4 5-6 5 6 4-4-2 12H5z"), Shape::Path("M5 19h14")];
const PALETTE: &[Shape] = &[Shape::Path("M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.9 0 1.65-.75 1.65-1.69 0-.44-.18-.83-.44-1.12-.29-.29-.44-.65-.44-1.13a1.64 1.64 0 0 1 1.67-1.67h2c3.05 0 5.55-2.5 5.55-5.55C22 6 17.46 2 12 2z"), Shape::Circle(13.5, 6.5, 0.6), Shape::Circle(17.5, 10.5, 0.6), Shape::Circle(8.5, 7.5, 0.6), Shape::Circle(6.5, 12.5, 0.6)];
const MUSIC: &[Shape] = &[Shape::Path("M9 18V5l12-2v13"), Shape::Circle(6.0, 18.0, 3.0), Shape::Circle(18.0, 16.0, 3.0)];
const CAMERA: &[Shape] = &[Shape::Path("M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z"), Shape::Circle(12.0, 13.0, 3.0)];
const GLOBE: &[Shape] = &[Shape::Path("M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20M2 12h20"), Shape::Circle(12.0, 12.0, 10.0)];
const MOON: &[Shape] = &[Shape::Path("M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9z")];
const SUN: &[Shape] = &[Shape::Path("M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"), Shape::Circle(12.0, 12.0, 4.0)];
const FLAME: &[Shape] = &[Shape::Path("M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.07-2.14-.22-4.05 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.15.43-2.29 1-3a2.5 2.5 0 0 0 2.5 2.5z")];
const SNOWFLAKE: &[Shape] = &[Shape::Path("M2 12h20M12 2v20M20 16l-4-4 4-4M4 8l4 4-4 4M16 4l-4 4-4-4M8 20l4-4 4 4")];
const UMBRELLA: &[Shape] = &[Shape::Path("M12 2v1M22 12a10 10 0 0 0-20 0zM12 12v8a2 2 0 0 0 4 0")];
const ANCHOR: &[Shape] = &[Shape::Path("M12 22V8M5 12H2a10 10 0 0 0 20 0h-3"), Shape::Circle(12.0, 5.0, 3.0)];
const COMPASS: &[Shape] = &[Shape::Path("m16.24 7.76-2.12 6.36-6.36 2.12 2.12-6.36z"), Shape::Circle(12.0, 12.0, 10.0)];
const PUZZLE: &[Shape] = &[Shape::Path("M10 3a2 2 0 0 1 4 0v1h4a1 1 0 0 1 1 1v4h1a2 2 0 0 1 0 4h-1v4a1 1 0 0 1-1 1h-4v-1a2 2 0 0 0-4 0v1H6a1 1 0 0 1-1-1v-4H4a2 2 0 0 1 0-4h1V5a1 1 0 0 1 1-1h4z")];
const ROBOT: &[Shape] = &[Shape::Path("M12 8V4H8"), Shape::Path("M2 14h2M20 14h2M15 13v2M9 13v2"), Shape::Rect(4.0, 8.0, 16.0, 12.0, 2.0)];
const CAT: &[Shape] = &[Shape::Path("M12 5c.67 0 1.35.09 2 .26 1.78-2 5.03-2.84 6.42-2.26 1.4.58-.42 7-.42 7 .57 1.07 1 2.24 1 3.44C21 17.9 16.97 21 12 21s-9-3-9-7.56c0-1.25.5-2.4 1-3.44 0 0-1.89-6.42-.5-7 1.39-.58 4.72.23 6.5 2.23A9.04 9.04 0 0 1 12 5z"), Shape::Path("M8 14v.5M16 14v.5M11.25 16.25h1.5L12 17z")];
const TREE: &[Shape] = &[Shape::Path("m17 14 3 3.3a1 1 0 0 1-.7 1.7H4.7a1 1 0 0 1-.7-1.7L7 14h-.3a1 1 0 0 1-.7-1.7L9 9h-.2A1 1 0 0 1 8 7.3L12 3l4 4.3a1 1 0 0 1-.8 1.7H15l3 3.3a1 1 0 0 1-.7 1.7H17z"), Shape::Path("M12 22v-3")];
const MOUNTAIN: &[Shape] = &[Shape::Path("m8 3 4 8 5-5 5 15H2L8 3z")];
const WAVE: &[Shape] = &[Shape::Path("M2 6c.6.5 1.2 1 2.5 1C7 7 7 5 9.5 5c2.6 0 2.4 2 5 2 2.5 0 2.5-2 5-2 1.3 0 1.9.5 2.5 1M2 12c.6.5 1.2 1 2.5 1 2.5 0 2.5-2 5-2 2.6 0 2.4 2 5 2 2.5 0 2.5-2 5-2 1.3 0 1.9.5 2.5 1M2 18c.6.5 1.2 1 2.5 1 2.5 0 2.5-2 5-2 2.6 0 2.4 2 5 2 2.5 0 2.5-2 5-2 1.3 0 1.9.5 2.5 1")];
const CLOCK: &[Shape] = &[Shape::Path("M12 6v6l4 2"), Shape::Circle(12.0, 12.0, 10.0)];
const KEY: &[Shape] = &[Shape::Path("M2.59 17.41A2 2 0 0 0 2 18.83V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.17a2 2 0 0 0 1.42-.59l.81-.81a6.5 6.5 0 1 0-4-4z"), Shape::Circle(16.5, 7.5, 0.6)];
const SHIELD: &[Shape] = &[Shape::Path("M20 13c0 5-3.5 7.5-7.66 8.95a1 1 0 0 1-.67-.01C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.24-2.72a1.17 1.17 0 0 1 1.52 0C14.51 3.81 17 5 19 5a1 1 0 0 1 1 1z")];
const GIFT: &[Shape] = &[Shape::Path("M12 8v13M19 12v7a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-7M7.5 8a2.5 2.5 0 0 1 0-5A4.8 8 0 0 1 12 8a4.8 8 0 0 1 4.5-5 2.5 2.5 0 0 1 0 5"), Shape::Rect(3.0, 8.0, 18.0, 4.0, 1.0)];
const GAMEPAD: &[Shape] = &[Shape::Path("M6 11h4M8 9v4M15 12h.01M18 10h.01"), Shape::Path("M17.32 5H6.68a4 4 0 0 0-3.98 3.59c-.01.05-.01.1-.02.15C2.6 9.42 2 14.46 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.41-1.41A2 2 0 0 1 9.83 16h4.34a2 2 0 0 1 1.42.59L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.55-.6-6.58-.68-7.26-.01-.05-.01-.1-.02-.15A4 4 0 0 0 17.32 5z")];
const CHECKALL: &[Shape] = &[Shape::Path("M2 12l4 4 8-10"), Shape::Path("M9 13l4 4 9-11")];

pub static ICONS: &[(&str, &[Shape])] = &[
    ("sliders", SLIDERS),
    ("swap", SWAP),
    ("refresh", REFRESH),
    ("clip", CLIP),
    ("copy", COPY),
    ("stop", STOP),
    ("star", STAR),
    ("x", X),
    ("check", CHECK),
    ("eye", EYE),
    ("bulb", BULB),
    ("languages", LANGUAGES),
    ("doc", DOC),
    ("library", LIBRARY),
    ("natural", NATURAL),
    ("literal", LITERAL),
    ("qmark", QMARK),
    ("nb", NB),
    ("male", MALE),
    ("female", FEMALE),
    ("hat", HAT),
    ("dash", DASH),
    ("smile", SMILE),
    ("winMin", WINMIN),
    ("winMax", WINMAX),
    ("winRestore", WINRESTORE),
    ("undo", UNDO),
    ("redo", REDO),
    ("bold", BOLD),
    ("italic", ITALIC),
    ("underline", UNDERLINE),
    ("strike", STRIKE),
    ("ul", UL),
    ("ol", OL),
    ("quote", QUOTE),
    ("alignL", ALIGNL),
    ("alignC", ALIGNC),
    ("alignR", ALIGNR),
    ("alignJ", ALIGNJ),
    ("link", LINK),
    ("eraser", ERASER),
    ("filenew", FILENEW),
    ("folder", FOLDER),
    ("save", SAVE),
    ("shorten", SHORTEN),
    ("lengthen", LENGTHEN),
    ("summarize", SUMMARIZE),
    ("proofread", PROOFREAD),
    ("synonyms", SYNONYMS),
    ("wand", WAND),
    ("plus", PLUS),
    ("pencil", PENCIL),
    ("cpu", CPU),
    ("chev", CHEV),
    ("arrowR", ARROWR),
    ("upload", UPLOAD),
    ("download", DOWNLOAD),
    ("docs", DOCS),
    ("bolt", BOLT),
    ("graduation", GRADUATION),
    ("sparkles", SPARKLES),
    ("feather", FEATHER),
    ("briefcase", BRIEFCASE),
    ("book", BOOK),
    ("heart", HEART),
    ("coffee", COFFEE),
    ("megaphone", MEGAPHONE),
    ("leaf", LEAF),
    ("rocket", ROCKET),
    ("crown", CROWN),
    ("palette", PALETTE),
    ("music", MUSIC),
    ("camera", CAMERA),
    ("globe", GLOBE),
    ("moon", MOON),
    ("sun", SUN),
    ("flame", FLAME),
    ("snowflake", SNOWFLAKE),
    ("umbrella", UMBRELLA),
    ("anchor", ANCHOR),
    ("compass", COMPASS),
    ("puzzle", PUZZLE),
    ("robot", ROBOT),
    ("cat", CAT),
    ("tree", TREE),
    ("mountain", MOUNTAIN),
    ("wave", WAVE),
    ("clock", CLOCK),
    ("key", KEY),
    ("shield", SHIELD),
    ("gift", GIFT),
    ("gamepad", GAMEPAD),
    ("checkAll", CHECKALL),
];
