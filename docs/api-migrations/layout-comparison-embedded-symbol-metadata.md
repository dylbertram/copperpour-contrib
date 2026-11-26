### `compare_schematic_layout` checks embedded symbol exports (patch release)

When comparing two saved snapshots, `compare_schematic_layout` now reports `blocked`
if a retained symbol changes its embedded library identity (`lib_name`), jumper
settings, unit metadata, or additional library fields or pins. This includes metadata
for unplaced units and library-only changes, even if the placed pins stay on the same
nets. The reasons name the affected symbol and metadata. Use `verdict`, not the reason
text, to decide whether an edit can proceed.

The comparison still reads only the two saved sheets. It does not run KiCad, read
external libraries, or change connectivity decisions. Symbol drawings and pin-text
styling are not library export metadata. Moving an unplaced pin can still change the
exported order of pins within its unit.

Use KiCad-saved snapshots. With conflicting duplicate pin numbers, the comparison
can block a pin-list edit when it cannot prove that KiCad exports the same duplicate.
The same applies when distinct pin numbers tie in KiCad's sort order, such as `8`
and `08`. When distinct pins share a position, a pin edit can block the comparison
because KiCad may change the exported order. If pin numbers also tie in KiCad's
sort, even an edit in a different unit can affect that order. Stacked-pin notation
is expanded for comparison, except when a record would expand to more than 4,096
numbers. Those records are compared as saved text. Changing the spelling of a stack
can still reorder library pin records. When saved placements identify the units of a
component unambiguously, a secondary local definition contributes its placed unit's
metadata, including other pin records that can affect its sort.
