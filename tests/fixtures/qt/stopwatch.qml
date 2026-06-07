// QT-05a fixture: a Stopwatch screen with an attached scjson side-file
// (stopwatch.scjson). Exercises QT-05a's `<basename>.qml` ↔
// `<basename>.scjson` discovery rule.
//
// The inline structure here is plain QT-04 vintage (Buttons with
// onClicked, no States {} block) — QT-05d will introduce inline
// state-machine authoring. For QT-05a the state machine lives
// next door as a hand-authored .scjson; ingest wires the two
// together via UiModule.state_machine.

import QtQuick 2.15
import QtQuick.Controls 2.15

Item {
    id: root
    width: 320
    height: 200
    property string title: "Stopwatch"

    Label {
        id: display
        x: 0
        y: 0
        width: 320
        height: 60
        text: root.title
    }

    // QT-05c §5: text bound to the istate DataModel. The bound
    // Label updates after a caller does
    // `machine.borrow_mut().dm.elapsed = …; refresh_bindings(...)`.
    Label {
        id: counter
        x: 0
        y: 60
        width: 320
        height: 30
        text: sm.dm.elapsed
    }

    Button {
        id: startBtn
        x: 0
        y: 100
        width: 100
        height: 60
        text: "Start"
        onClicked: dispatch("start")
    }

    Button {
        id: stopBtn
        x: 110
        y: 100
        width: 100
        height: 60
        text: "Stop"
        onClicked: dispatch("stop")
    }

    Button {
        id: resetBtn
        x: 220
        y: 100
        width: 100
        height: 60
        text: "Reset"
        onClicked: dispatch("reset")
    }
}
