// QT-04c fixture: a Label whose `text:` is bound to a root-scope
// `string` property. Exercises the initial-value binding path —
// the label constructor reads `state.title.clone()` once at build
// time.

import QtQuick 2.15

Item {
    id: root
    property string title: "Greetings"
    width: 320
    height: 80

    Label {
        text: title
        anchors.fill: parent
    }
}
