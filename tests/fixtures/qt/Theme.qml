// QT-06 fixture: canonical theme authoring convention.
// Exercises QT-06 §5 / §6: a QtObject root with property
// declarations covering colors, spacing, radii, fonts, and
// dark-mode overrides. `qt emit-tokens` walks these and
// produces the matching tokens.yaml.

import QtQuick 2.15

QtObject {
    // Color tokens
    property color primary:    "#3366ff"
    property color background: "#ffffff"
    property color text:       "#111111"
    property color accent:     "#ff8800"

    // Spacing tokens
    property int spacing_xs: 2
    property int spacing_sm: 4
    property int spacing_md: 8
    property int spacing_lg: 16
    property int spacing_xl: 24

    // Radius tokens
    property int radius_none: 0
    property int radius_sm:   2
    property int radius_md:   4
    property int radius_lg:   8
    property int radius_full: 255

    // Font tokens
    property string font_small:   "tiny"
    property string font_body:    "default"
    property string font_heading: "bold"

    // Dark-mode color overrides (suffix _dark)
    property color background_dark: "#171923"
    property color text_dark:       "#f7fafc"
}
