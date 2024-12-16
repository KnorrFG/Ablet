# Todo

publish crate
document
  - REadme
  - rust doc

# Things that need to be taken care of

- Inefficiency of AText type
- selections
- consider supporting windows line endings, or at least add a warning?
- multiline editor / vim and emacs
- find buffer and get buffer coord for click
- linewrap
- more examples
  - for something like an email reader, a function that gets a line number
  from a cursor pos would be usefull, and a fn that gets a line range from a
  cursor pos
- Associate a value: T with a buffer that can be returned to identify the buffer.
  the buffer map should probably have a function that takes a screen coord and 
  returns a buffer id and a buffer coord
- expose crossterm event
- add event processor  
