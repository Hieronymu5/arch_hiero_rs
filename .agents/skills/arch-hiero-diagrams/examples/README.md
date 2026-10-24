# Examples

Each file holds the `create_diagram` input and the `edit_diagram` ops that build one diagram. Where `edit_diagram` is a list of two batches, send them as two calls. A value such as `"{Perimeter}"` is the id of the element with that name, from the result of the call before, and `"{H-WEST/Perimeter}"` the one inside H-WEST. Every other id is a `ref` from the same batch.

## `conceptual-order-entry.json`

A user places an order in Order Entry through the web; Order Entry calls Inventory's API; a job pulls orders from Order Entry's database into the Prometheus data lake, and Order Entry keeps an Orders CSV file.

Top to bottom: the user and the downstream systems, which take Order Entry's data (the job and Prometheus); the central application, Order Entry (x 40, 420 wide: 40–460); then the upstream system it gets data from, Inventory.

- The job (160 wide) at x 280, Prometheus (200 wide) at x 260, and Orders DB inside Order Entry (120 wide) at x 300 share the centre x 360, so their arrows run straight without help. That keeps the job's arrow to the database clear of Order Entry's name and description, centred at x 250 in its header.
- The user (120 wide) at x 40 has its centre at x 100, but its arrow would meet the middle of Order Entry's top, at x 250. `"manualOffset": 60` (100 − 40) runs it straight down from the user.
- Inventory (200 wide) at x 140 has its centre at x 240. `"manualOffset": 200` (240 − 40, from Order Entry's left edge) runs Order Entry's arrow straight down into it.
- Order Entry is resized to 180 tall, for its 60 header and the 80-tall figures with padding, before anything goes inside it.

## `physical-3-tier.json`

A 3-tier web application (HTTP reverse proxy, Spring Boot, MySQL) with failover across two data centres, H-EAST and H-WEST, each its own network.

- The first batch renames the Intranet network to H-EAST, makes its three zones 320 tall (two figures stacked 160 apart, with an arrow between), and adds H-WEST at x 760. H-WEST copies H-EAST's zones and their sizes, and the End User zone widens to span both.
- The second batch uses the zone ids from the first one's result. One column per data centre, every figure at the same x, so each arrow runs straight down: figures are 120 × 100, at x 280 in H-EAST (centre 340) and x 1000 in H-WEST (centre 1060).
- The customer, at x 640 (centre 700), sits halfway between the two columns. Its two arrows can't both be straight.
- `instanceCount` draws one figure with "×2" beside it. The databases are two separate figures, not one with "×2", because the replication arrow between the primaries runs sideways and would cross the badge. The primaries share a y, so that arrow runs straight across.

## `deployed-component-web-app.json`

Where each component of a web application runs, by network zone. Layers stack top to bottom in the order they're added, and components sit side by side in their layer in the order they're added. Both size themselves, so there are no positions or sizes to give. Okta, a third-party service, is an `external` dependency, drawn red.
