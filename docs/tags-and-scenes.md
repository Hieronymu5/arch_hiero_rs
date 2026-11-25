# Tags and scenes

**Applies to:** every diagram type · **Rust:** `hiero-core::tags`, `hiero-editor` (`editor/tags.rs`),
`hiero-app` (`tags_tab.rs`) · **Prototype source:** `utils/diagram-tags.ts`,
`utils/diagram-scenes.ts`, `common/diagram-editor/{tag-panel,scene-panel}.ts`,
`common/diagram-edit/{diagram-tags-tab,diagram-scenes-tab}.ts`

## Tags

A tag is a named set of a diagram's elements, shown or hidden together. A diagram's tags are
kept in play order. An element can be in any number of them:

- An element in no tag always shows.
- One in tags shows while any of them does, and only while what holds it shows too.
- An arrow shows only while both its ends do. While a scene shows, every arrow stays, faint
  while either end is (see Scenes).

Names are required, at most 40 characters, and unique within the diagram, ignoring case.

### On the diagram

The Tags panel sits under the palette, and shows only once the diagram has tags.

- **Each tag's row**, its eye and name, shows or hides its elements. While editing, hidden elements are drawn faint
  (30%) and can still be picked. While viewing, they aren't drawn and can't be clicked.
- **The eye beside the heading** shows every tag, or hides every one when all are showing. It's
  faint while only some show.
- **While editing, the toolbar's tag buttons apply a tag**, once the diagram has tags: the tag
  icon turns tagging on and off with the current tag, and the current tag's name beside it lists
  the tags to choose another, which turns tagging on. The current tag is the top one until
  another is chosen. While tagging, clicking an element, or pressing Enter or Space on one, puts
  it in the tag or takes it out, and selects or moves nothing; the tag's elements are ringed in
  the accent, and its name in the list is in the accent and bold. Pressing Escape, or empty
  canvas, stops tagging. Each tag's row in the list only shows or hides it.
- **While editing, the heading "Tags"** opens Settings on its Tags tab.
- **The selected element's properties** have a Tags section, above its arrows, with a checkbox
  for each tag, ticked for those it's in. Ticking one puts it in the tag, and unticking takes it
  out, as one undo step. It shows only once the diagram has tags.
- **Which tags are hidden isn't saved:** each visit starts with all of them shown.
- **A long name** is cut short with "…", whole in its hint.
- **Pasting** keeps only the tags the diagram has.

### Settings

A diagram's Settings page has six tabs down its left: Details,
Appearance, Sub Diagrams, Tags, Scenes, and About, with Back and Save Changes above them. The
tabs edit one draft: Save Changes, enabled once something has changed, saves every tab's changes
as one undo step and goes back to the diagram, and Back leaves without saving any. If a tab has
something that isn't valid, Save Changes shows that tab instead. Sub diagrams removed there are
deleted only when the changes are saved, and until then are marked, with Undo to keep them.
About shows when the diagram was created and last updated.

The Tags tab lists each tag with its name in a field, typed over to rename it, how many elements
are in it, and buttons to move it earlier or later, or delete it. A field adds one. Deleting a
tag asks first when it has elements, or when a scene shows only it; then its elements are taken
out of it, and those scenes are deleted, once the changes are saved.

## Scenes

A scene is a named view that shows the elements of one or more of the diagram's tags, and hides
the rest of the tags' elements. Elements in no tag show in every scene. Scene names follow the
rules for tag names.

- **Settings, Scenes tab:** lists each scene with its name in a field and the tags it shows as
  checkboxes, changed in place, its buttons (move up, move down, delete) on one line and its tags
  wrapping. A form after the list adds one (a name, and checkboxes for the tags it shows).
- **Viewing a diagram with scenes:** a Scenes panel takes the Tags panel's place. It has
  Previous, Play or Pause, and Next, then the scenes, with the one showing highlighted. Viewing
  starts at the first scene.
- **What a scene doesn't show** stays very faint (8%) and can't be clicked, and so does every
  arrow to or from it. All the arrows are routed together, so none moves as the scenes step
  on. The first scene shown is drawn at once.
- **As the scenes step on,** the camera eases (slow, fast, slow, over 0.9 s) to frame what the
  scene adds: what it shows that another scene hides, as what every scene shows would keep the
  camera still, or all it shows when that's nothing, or all that's drawn, legend included, when
  it hides nothing. It's in the middle of the view, as large as fits 40 points from the edges,
  up to 150%, but no further than the view can go, so it doesn't stall at the drawing's edge.
  When the view is there already (within a pixel), it doesn't move. Scrolling or zooming stops
  the camera, and its zoom isn't remembered as the user's own. Once the camera is there, 0.1 s
  later (or at once if it didn't move), the elements the scene takes away fade to faint in
  0.3 s, with their arrows, and what it brings in is built from the top down: a line moves down
  the canvas from the topmost new element or arrow, at 350 px a second, or faster so it
  reaches the lowest in 1.4 s. Each element fades in whole over 0.35 s as the line reaches its
  top, and each arrow, with its connector and text, faint until the line reaches its top, is
  then drawn down as the line passes, with nothing of it showing below the line. The fades follow
  CSS's `ease`. What both scenes show stays as it is. The prototype skips all this for reduced
  motion, which egui can't tell.
- **The first scene shown,** before the scenes have stepped on, scrolls the leftmost
  section it's about into view, if it isn't already: a section in one of its tags, or one over
  an element it shows of its tags that reaches under no other section. Its left end shows when
  it's too wide to fit.
- **Play** steps on every 3 s and stops at the last scene. Play at the last scene starts again
  from the first. Choosing a scene, Previous, or Next stops play.

## Stored data

```jsonc
// diagram
{ "tags": [{ "id": "phase-1", "name": "Phase I" }],
  "scenes": [{ "id": "s1", "name": "Phase I", "tags": ["phase-1"] }] }
// element
{ "tags": ["phase-1"] }
```

Each list is left out when it's empty.

- **Storage:** tags and scenes are part of the diagram's content, so they're in versions and the
  content hash. They're kept in the `tags` and `scenes` columns, as JSON. An element's tags are
  in its props.
- **File check:** refuses an empty list, a duplicate id or name, a name over 40 characters, an
  unknown field on a tag or scene, a scene with no tags, and an element or scene naming a tag the
  diagram doesn't have, or the same one twice.
- **MCP:** `update_diagram_details` takes `tags` and `scenes`, each replacing the whole list. Give
  a tag its id to keep it, or leave the id out to add one; a tag left out is deleted.
  `edit_diagram`'s `add_element` and `update_element` take `tags`, by id or name.
