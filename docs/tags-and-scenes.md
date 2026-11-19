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
- An arrow shows only while both its ends do.

Names are required, at most 40 characters, and unique within the diagram, ignoring case.

### On the diagram

The Tags panel sits under the palette, and shows only once the diagram has tags.

- **Each tag's eye** shows or hides its elements. While editing, hidden elements are drawn faint
  (30%) and can still be picked. While viewing, they aren't drawn and can't be clicked.
- **The eye beside the heading** shows every tag, or hides every one when all are showing. It's
  faint while only some show.
- **While editing, clicking a tag's name applies it.** Clicking an element, or pressing Enter or
  Space on one, then puts it in the tag or takes it out, and selects or moves nothing. The tag's
  elements are ringed in the accent. Clicking the name again, or pressing Escape, stops applying.
- **While editing, the heading "Tags"** opens Edit Details on its Tags tab.
- **Which tags are hidden isn't saved:** each visit starts with all of them shown.
- **Pasting** keeps only the tags the diagram has.

### Edit Details

Edit Details has four tabs down its left: Details, Sub Diagrams, Tags, and Scenes, with Back
above them.

The Tags tab lists each tag, with how many elements are in it, and can rename it, move it earlier
or later, or delete it. A field adds one. Deleting a tag asks first when it has elements, or when
a scene shows only it; then its elements are taken out of it, and those scenes are deleted. Each
change applies at once, as one undo step.

## Scenes

A scene is a named view that shows the elements of one or more of the diagram's tags, and hides
the rest of the tags' elements. Elements in no tag show in every scene. Scene names follow the
rules for tag names.

- **Edit Details, Scenes tab:** lists each scene with the tags it shows. A scene can be edited,
  moved, or deleted, and a form adds one or edits one (a name, and checkboxes for the tags it
  shows).
- **Viewing a diagram with scenes:** a Scenes panel takes the Tags panel's place. It has
  Previous, Play or Pause, and Next, then the scenes, with the one showing highlighted. Viewing
  starts at the first scene.
- **What a scene doesn't show** stays very faint (8%) and can't be clicked. When a later scene
  shows it, it fades in over 0.8 s, with its arrows. The first scene shown is drawn at once.
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
