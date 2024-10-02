# Committee Beacon

The `CommitteeBeacon` component participates in a commit-reveal process that provides randomness for selecting the next committee. While it does not directly select and save the next committee to application state, it provides the application state executor with a secure and fair random seed for making the selection.

```mermaid
stateDiagram-v2
		WaitForEpochChange: Wait for Epoch Change
		CommitteeSelectionBeacon: Committee Selection Random Beacon
		ExecuteEpochChange: Execute Epoch Change
    [*] --> WaitForEpochChange
    WaitForEpochChange --> CommitteeSelectionBeacon: [Epoch change transactions from > 2/3 committee]
    state CommitteeSelectionBeacon {
        [*] --> Commit
        Commit --> Commit: [Commit period timeout with<br />insufficient participation]
        Commit --> Reveal: [Commit period timeout with<br />sufficient participation]
        Reveal --> Commit: [Reveal period timeout]
        Reveal --> [*]: [All commits revealed]
    }
    CommitteeSelectionBeacon --> ExecuteEpochChange
    note right of ExecuteEpochChange
        Derives random seed<br />for committee selection<br />from beacon reveals
    end note
    ExecuteEpochChange --> [*]
```
