// QT-04d fixture: a transparent MouseArea covering the parent
// fires a state mutation on click. Exercises ClickArea lowering +
// QT-04b's body grammar.

import QtQuick 2.15

Item {
    id: root
    width: 200
    height: 100
    property int taps: 0

    MouseArea {
        id: hit
        anchors.fill: parent
        onClicked: taps += 1
    }
}
