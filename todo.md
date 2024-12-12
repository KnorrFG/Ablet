# Things to implement

## First Steps

The things i wanted to build easily:
- an async chat
- a way to spawn processes and watch output in parallel
- something like a mailclient

## Next Todos:

- Prompt commands
  - insert char
  - delete at end

- Prompt loop
  - confirm

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
