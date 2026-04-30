// QT-04f fixture: a non-root id'd item declares a property that a
// sibling's onClicked handler decrements. Exercises namespaced
// ScreenState fields and the multi-scope resolution walk.

import QtQuick.Controls 2.15

Item {
    id: app
    width: 200
    height: 100

    Rectangle {
        id: bg
        property int alpha: 100
        x: 0
        y: 0
        width: 200
        height: 50
    }

    Button {
        id: dim
        text: "Dim"
        x: 0
        y: 50
        width: 200
        height: 50
        onClicked: bg.alpha -= 10
    }
}
