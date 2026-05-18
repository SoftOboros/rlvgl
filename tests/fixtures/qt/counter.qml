// QT-04b fixture: a Button declaring its own root-level property
// `count` and an onClicked handler whose body matches the §7 grammar.
// Exercises ScreenState struct emission + state mutation lowering.

import QtQuick.Controls 2.15

Button {
    id: root
    property int count: 0
    text: "Press me"
    width: 200
    height: 80
    onClicked: count += 1
}
