// QT-05d fixture: inline `states:` / `transitions:` blocks.
// Exercises QT-05d §5 / §6: walking these declarations into
// `qt_scjson::Scxml` and writing them to a sibling `.scjson`.
// QT-05a then re-ingests the produced file (round-trip parity
// per QT-05d §8).

import QtQuick 2.15

Item {
    id: root
    width: 320
    height: 200

    states: [
        State { name: "idle"; initial: true },
        State { name: "running" }
    ]

    transitions: [
        Transition { from: "idle"; to: "running"; event: "start" },
        Transition { from: "running"; to: "idle"; event: "stop" }
    ]
}
