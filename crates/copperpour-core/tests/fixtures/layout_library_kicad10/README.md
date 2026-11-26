# Embedded symbol export metadata fixtures

Every `metadata.kicad_sch` is the output of KiCad 10.0.8
`kicad-cli sch upgrade --force`. A second force-resave reproduced the bytes
except for the tied secondary-alias case described below.
No project file is required. All variants use the same basename and preserve the
sheet and retained symbol UUIDs. The comparison tests copy the files into separate
directories and assert that the calls change neither the contents nor the entries.

## Provenance

The common input came from `../pin_location_mirror_kicad10.kicad_sch`, a KiCad-resaved
sheet made from the stock `Amplifier_Operational:LM358` symbol. The pre-resave draft
kept only U1 unit A and added local labels to pins 1 and 2. Each other directory
changes one item in a copy of that draft before force-resave. In `lib-name`, the
embedded key and nested symbol names were renamed together and U1 uses the new key
via `lib_name`. Its placed `lib_id` remains `Amplifier_Operational:LM358`.

Both KiCad 10.0.6 and 10.0.8 exported each fixture with
`kicad-cli sch export netlist --format kicadxml`. The following deltas exclude the
volatile `<design>` header. The net records were equal for every pair in the table.

| Directory | One edit | XML delta |
| --- | --- | --- |
| `duplicate-jumper` | Enable the jumper flag | Component `duplicate_pin_numbers_are_jumpers` |
| `jumper-group` | Group pins 4 and 8 of unplaced unit C | Component `jumper_pin_groups` |
| `jumper-group-repeated` | Duplicate that group | Second component group |
| `unit-name` | Name unit B `Secondary` | Component unit B name |
| `unit-pin-number` | Renumber unit B's pin 7 to 17 | Component unit B pins and library pin numbers |
| `pin-unit-x-order` | Change the X order of unit C's pins | Component unit C pin order |
| `pin-unit-y-order` | Change the Y order at a shared X in unit C | Component unit C pin order |
| `pin-unit-membership` | Move unit B to unit slot 4 | Component unit count and pin lists |
| `libpart-field` | Add library-only `AuditVendor` | Library part field |
| `libpart-datasheet` | Change only the embedded Datasheet | Library part field and `docs` |
| `libpart-pin-name` | Rename unit C's pin 8 | Library part pin name |
| `libpart-pin-type` | Change unit C's pin 8 type | Library part pin type |
| `duplicate-library-key` | Append another LM358 with pin 8 renamed | Library part pin name from the second definition |
| `duplicate-pin-before` and `duplicate-pin-after` | Move one duplicate of unit C's pin 8 across the other | Library part pin name |
| `duplicate-pin-roster-before` and `duplicate-pin-roster-after` | Duplicate unit C's pin 11 without changing its text | Library part pin 8 name |
| `lib-name` | Use the equivalent local `LM358_1` | Component `libsource`, library part identity and libraries |
| `alias-pin-type` | Change unit C's pin 8 type in `LM358_1` | Library part pin type, compared with `lib-name` |
| `drawing` | Change a body outline stroke | None |
| `pin-text-style` | Change pin name font size | None |
| `pin-style` | Change pin 1's graphic shape | None |
| `pin-unit-order` | Move pin 8 without reordering the unit pins | None |
| `unit-default-name` | Set the unit B name explicitly to `B` | None |
| `common-parity-before` and `common-parity-after` | Move the only unit's pins to the common block | None |
| `unit-zero-name` | Name the common block in `common-parity-after` | None |
| `empty-pin-before` and `empty-pin-after` | Move a pin with an empty number within unit C | Component unit C pin order |
| `zero-before` and `zero-after` | Add another `8` alongside `8` and `08` in alternative body styles | Extra `<libparts>` pin record |
| `stack-before` and `stack-after` | Spell the same stack as `[8,9]` and `[8-9]` | None |
| `secondary-base` and `secondary-b` | Name placed unit B in its own local alias | Component unit B name |
| `secondary-base` and `secondary-c` | Name unplaced unit C in B's local alias | None |
| `secondary-base` and `secondary-key` | Switch B back to the base library key | None |
| `tie-before` and `tie-swap` | Swap the saved order of `08` and the second `8` in unit C without moving either | One fewer `<libparts>` pin record |
| `tie-before` and `tie-after` | Move `08` on X without changing the exported pin lists | None |
| `stack-sort-before` and `stack-sort-after` | Move pin 24 by 0.01 mm away from pin 23 in unit C | Pin 23 and 24 exchange positions in the component's unit C pin list |
| `same-number-before` and `same-number-after` | Move an identical pin 8 away from another pin 8 at the same position | None |
| `short-alias-before` and `short-alias-after` | Name unit B in the primary definition when the placed B alias has no unit B slot | Component unit B name |
| `phantom-0-before` and `phantom-0-after` | Move common pin 8 with a second placement using unit zero | Component unit C pin order |
| `phantom-4-before` and `phantom-4-after` | Move common pin 8 with a second placement above the primary definition's unit count | Component unit C pin order |
| `path-ref-before` and `path-ref-after` | Name unit C in an alias saved with a different reference in a second project | None in the current project |
| `path-unit-before` and `path-unit-after` | Name unit A in an alias saved with a different unit in a second project | None in the current project |
| `path-set-before` and `path-set-after` | Name unit C in an alias with an unmatched saved project path | None in the current project |
| `alias-tie-before` and `alias-tie-after` | Add the seventeenth pin to unplaced C in the B alias | `8` and `08` exchange order in the component's unit B pin list |
| `overlap-stacks-before` and `overlap-stacks-after` | Change `[8,9]` to `[8-9]` alongside an overlapping `[8,10]` | Reordered `<libparts>` pin records with the same record count |

Drawing-only parity is an export claim, not an ERC-cleanliness claim. KiCad can
report a stock library mismatch after an embedded edit, including a body stroke edit.

KiCad collapsed the duplicate embedded key to the second definition on save. In the
duplicate-pin pair, it also regenerated a UUID for a second, unplaced pin 8 record.
For the roster pair, the seed also contains seven extra pins in unit C. Adding
another pin 11 leaves the unit pin list equal but changes which pin 8 is exported.

The secondary-alias fixtures restore U1 unit B with a separate embedded key and
name all three placed pins with labels to hold the native net names fixed. U1 unit A
has the earlier UUID. It supplies the component source and the default unit slots;
the placed B alias supplies only its own unit slot.

The tied-number fixtures add two pins to unit C, `08` and another `8`, before
resaving. The stacked-sort fixtures add pins 9 to 25 to unit C. Pins 11 and 21
share one position, and pins 23 and 24 share another before pin 24 moves. The
short-alias pair removes the B alias's last two units before resaving both sheets.
The same-number fixtures reuse the tied-number seed with three identical pin 8
records. Moving one of two coincident copies changes neither exported list.

The phantom-slot pairs give units A and B their own earlier copies of the common
pin numbers and leave unit C with only common pins. The secondary placement has
unit zero or unit four. KiCad warns about annotation but exports the reordered
pins only in unit C. The three saved-path pairs start from `secondary-base` and
add records for another project. Using that project's root UUID makes the alias's
renamed slot appear in the exports for `path-ref` and `path-unit`. The comparison
also checks the whole alias for `path-set` because the two placements' saved
contexts do not match.

The tied secondary-alias fixtures each use one force-resave. The saved unit B
blocks are identical. A second save can swap the two equivalent sort keys in the
17-pin definition. The fixtures freeze the first save, so the CLI exports and the
served regression start with the same bytes.

### Board reader versus raw export

The KiCad 10.0.8 board netlist reader passes jumper settings and the nonempty unit
pin lists to the board updater. It discards empty unit pin numbers, so the
`empty-pin` pair changes raw XML and S-expression output without demonstrating a
board update. The reader skips `<libparts>` fields and individual pin attributes;
it uses the footprint filters and counts pin records for downstream consumers.
