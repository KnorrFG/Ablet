# Things to implement

## First Steps

The things i wanted to build easily:
- an async chat
- a way to spawn processes and watch output in parallel
- something like a mailclient

## Next Todos:


- KeyHandler type, basically the closure in edit_prompt, but as a custom trait object.
  This way we can implement vim vs emacs hotkeys vs user hotkeys, and it should then
  be a mut ref, I guess, so it can be called again
- also I guess there is no reason for a special prompt buffer. At least there wont be,
  if I allow buffers of a fixed height.
- render something colorful for testing.

## Developer doku and contributor infos


## Longterm Goals and Ideas

- Document API: replace_range 
- input - prompt - keyboard:
  - low level:
    - (line) editor - Actions:
      - insert char at cursor
      - move cursor
        - start 
        - end
        - fw
        - bw
      - delete
        - char fw
        - char bw
        - word bw
        - till end of line
        - till start of line
        - line
      - clear line

- cursor support in buffer for navigation.
  - buffer commands for moving cursor
  - functions to move cursor and command interpreter that calls functions

- input mouse: 
  - buttons? Special type of attribute? This is a higher level

- 2 modes: event processing, (normal mode) and insert mode

## Things that need to be taken care of

- Inefficiency of AText type
- selections
- consider supporting windows line endings, or at least add a warning?
- the raw view is really just a special case of the fancy view, and
  it might be a better idea to just have one view type
