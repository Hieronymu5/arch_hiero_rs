# Physical diagram

**Type id:** `physical` · **Has arrows:** yes (protocol + port) · **Custom colours:** yes (12)
**Rust crate:** `crates/diagrams/hiero-physical`
**Prototype source:** `arch_hiero/src/app/diagrams/physical/`, `model/physical-diagram.ts`,
`utils/physical-figures.ts`, `utils/physical-diagram.ts`

Shows the infrastructure: networks divided into zones, the users, servers, databases, appliances,
and storage inside them, and the traffic between them with its protocol and port.

## Elements

The figure kinds take a 120×100 area: a 40 px tall figure at the top, with room for a name of up
to two lines underneath. Each figure's name, and a server's or database's instance count, is
in the *labels* colour with no background, so whatever is behind it, such as its zone, shows
through, in the editor and the SVG and draw.io exports. Figures are drawn on top of zones and
networks (`onTop`).

| Kind | Label | Figure | Fill | Can contain | Extra properties |
|---|---|---|---|---|---|
| `user` | User | Person (same as the conceptual actor) | users | – | – |
| `server` | Server | Tall box with three lines (physical) / cube (virtual machine) / container ship carrying squares (container) | servers | – | Server type, Instance count |
| `database` | Database | Cylinder with an abbreviation (PSQL, MySQL, ORCL, Mongo, MSSQL, Cass, DB) | data | – | Database type, Node count |
| `firewall` | Firewall | Brick wall | appliances | – | – |
| `load-balancer` | Load Balancer | Circle with a small circle on its left joined to three on its right | appliances | – | – |
| `gateway` | Gateway | Box with three small ports on its left | appliances | – | – |
| `storage` | Storage | Three stacked disks | data | – | – |
| `network` | Network | Square region, name top-left, 2 px border | networks | zone | – |
| `zone` | Zone | Square region, name bottom-left, 2 px border, 20 px header and footer | zones | user, server, database, load-balancer, gateway, storage | Colour (only when outside a network) |

Every figure kind (user through storage) can have arrows to every other figure kind.

**Instances:** when a server's instance count or a database's node count is more than 1, it's
shown in a small black box with round ends and white text beside the figure's bottom-right
corner, level with its bottom (never lower), and far enough right to clear the head of an arrow
coming in at the figure's right (a physical server's and a container ship's move out past it): "3" (a circle), "50+", or "50-100". A count can also be a minimum or a range (new in Rust; the prototype shows "×3" to
the right of the figure).

**Database types:**

| Value | Label | Abbreviation | Default port |
|---|---|---|---|
| (unset) | Database | DB | 5432 |
| `postgresql` | PostgreSQL | PSQL | 5432 |
| `mysql` | MySQL | MySQL | 3306 |
| `oracle` | Oracle | ORCL | 1521 |
| `mongodb` | MongoDB | Mongo | 27017 |
| `mssql` | Microsoft SQL Server | MSSQL | 1433 |
| `cassandra` | Cassandra | Cass | 9042 |

**Zone colours** (only for a zone outside a network). The zone is drawn with rounded corners, a
dashed border, and its name top-left. The fills are reused from the deployed component
diagram's colours, so they live in `hiero-core::palette`.

| Value | Fill | Border/name |
|---|---|---|
| blue (default, unset) | `#dae8fc` | `#2d5f9e` |
| `red` | `#f8cecc` | `#9c3530` |
| `yellow` | `#fff2cc` | `#806614` |
| `green` | `#d5e8d4` | `#356b24` |

## Layout rules (the hard part)

These are the type-specific `rules` hooks. They're all UI-free and unit-testable.

1. **New diagram:** starts with an **End User** zone across the top (at 40,40). Under it is an
   **Intranet** network divided into **Perimeter**, **Business**, and **Restricted** zones,
   each 600×200.
2. **Zones fill their network:** they're stacked from under the network's header, in order,
   across its full width, with the last zone taking any room left at the bottom. Whatever a zone
   holds moves with it.
3. **New network:** starts with copies of the zones in the first network on the diagram, at that
   network's size, or the default three zones if there's none. It's placed to the right of the
   last network, level with the first. The first network goes under anything stacked across the
   top, such as End User. Zones stacked above a new network (End User) are widened to span it.
4. **Zone dropped on a network:** divides the network, under its other zones.
5. **Resizing:** resizing a network shares its new height among its zones in proportion,
   snapped to the grid (`distribute`). Resizing a zone inside a network resizes the network to
   match. A network can't shrink past what its zones need to hold their contents.
6. **Moving zones:** a zone stays inside its network, across its full width, and only moves up
   and down. Dragging it takes the place in the stack where its middle lands, swapping with the
   zones it passes. Up and down arrow keys swap it with its neighbour. Left and right do
   nothing.
7. **Figures in zones:** an element dropped in a zone stays within the room the zone has (inside
   its padding, below its header, above its footer), so the zone and its network keep their
   size. If an arrow key moves an element that's already against a zone edge, it crosses into
   the next zone (or onto the canvas), within that zone's room.
8. **Firewalls stand alone:** they're never inside a zone or network, so one can sit across the
   edge between two zones or two networks, and doesn't move with them. A legacy firewall saved
   with a parent is drawn on its own.
9. **Removing** a network's last zone removes the network too.
10. **Zone firewalls:** a zone with **Firewall** ticked shows a firewall figure on its edge where
    each arrow from outside the zone enters it, centred on the line and drawn over it. It's two
    bricks tall, half a Firewall element's height, and has no name. An arrow drawn both ways
    counts as coming in. Arrows within the zone or going out don't get one. These firewalls
    aren't elements: they can't be selected or moved, and follow the arrows. They're in the SVG
    and draw.io exports too. (Not in the prototype.)

## Element properties

| Field | Applies to | Rule |
|---|---|---|
| Name | all | Required, at most 100 characters |
| Server type | server | Select: (Physical) / Virtual Machine / Container |
| Instance count | server | A whole number from 1 to 9999, a minimum such as `50+`, or a range such as `50-100` (first number smaller). Unset means 1. |
| Database type | database | Select: (Database (DB)) / the table above |
| Node count | database | As instance count |
| Color | zone outside a network | Select: (Blue) / Red / Yellow / Green |
| Firewall | zone | Checkbox: a firewall where each arrow comes in (rule 10) |
| Parent | figures that can be held | A select of the zones |
| Arrows list and Add arrow to | figure kinds | Same as conceptual |

## Arrows

- Routing is the shared `route_connections` (facing sides, straight where they overlap, else
  spread evenly). There's no manual straightening.
- An arrow at a figure's left or right side ends on its drawn outline (a database's cylinder, a
  virtual machine's cube; on a container ship, level with the ends of its hull), not on its box. Figures that can
  show an instance count take side arrows higher up (servers and container ships in the top
  32 px, databases and virtual machines in the top 24 px), so a level arrow and its head pass
  above the count. An end at a figure's bottom moves down to just under its name.
- Every arrow has a **protocol** and a **port**, both required and at most 20 characters. They
  appear in a tag on the middle of the line ("HTTPS:443"). The tag is placed so it doesn't cover
  figure names.
- A new arrow starts with the target's defaults:
  - a database: `TCP` on the database type's port
  - storage: `TCP 445` (SMB)
  - anything else: `HTTPS 443`
- Changing a database's type moves the arrows to it that use the old type's port to the new
  type's port. Arrows given another port keep it (new in Rust).
- Two arrows between the same elements in opposite directions, with the same protocol and port,
  are drawn as one line with a head at each end and one tag. Each is still its own arrow:
  clicking the line selects the first, and the other is selected from either element's list of
  arrows. Changing one's protocol or port draws them apart again (new in Rust).
- Removing an element removes its arrows.

## Colours

| Key | Default | Used for |
|---|---|---|
| `users` | `#d0e6fb` | User figure |
| `servers` | `#c5e0b4` | Server figure |
| `data` | `#ffe599` | Database and storage |
| `appliances` | `#f6c6d7` | Firewall, load balancer, and gateway |
| `networks` | `#e0e0e0` | Network fill |
| `zones` | `#f5f5f5` | Zone fill, and the background behind figure names |
| `arrows` | `#0066cc` | Arrow lines and heads |
| `protocols` | `#eeeeee` | Protocol/port tag fill |
| `lines` | `#333333` | Outlines of figures, networks, zones, and tags |
| `labels` | `#0b3d91` | Names under figures |
| `networkNames` | `#0b3d91` | Network names |
| `zoneNames` | `#0b3d91` | Zone names |

## Export

Uses the shared SVG and draw.io writers. Networks and zones are square regions with their names
in a corner, and the figures map to `PhysicalFigure` looks. The exports use the same outline and
arrow colours as the editor.

## Stored data

```jsonc
// element props
{ "serverType": "virtual-machine", "instanceCount": 3 }   // server
{ "databaseType": "postgresql", "instanceCount": 2 }      // database
{ "instanceCount": "50+" }  { "instanceCount": "50-100" }  // a minimum or a range is text
{ "zoneColor": "red", "firewall": true }                  // zone
// connection props
{ "protocol": "HTTPS", "port": "443" }
```

Rust shape: `Element<PhysicalKind, PhysicalProps>`, a fieldless kind enum with `PhysicalProps { database_type, server_type, instance_count, zone_color, firewall }` flattened beside it, and arrows as `Connection<PhysicalArrow { protocol, port }>`.

## Parity checklist

- [x] Starting elements on a new diagram
- [x] All 9 figures, including the 3 server variants and the database abbreviations
- [x] Zone stacking, swapping, proportional resize, and network copying zones
- [x] Figures confined to a zone's room. Arrow keys cross zone edges.
- [x] Firewall stands alone. Removing the last zone removes the network.
- [x] Zone colours outside networks
- [x] Zone firewalls on incoming arrows (beyond the prototype)
- [x] Arrows with protocol and port tags and the default rules
- [x] 12 custom colours
- [ ] SVG and draw.io export that match the prototype — *the writers are line-by-line ports of the prototype's and are unit-tested, but haven't yet been compared with files the prototype exported*
