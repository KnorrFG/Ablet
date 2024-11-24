# Usecase

* print to buffer
* get prompt 
  * either overlayed
  * in its own line buffer
  * or as new line in a buffer
* edit buffer
* split 
* resize splits
* get raw keys
* make it easy to define key combs
* get mouse clicks on labled text
* still be able to select and copy/paste text
* style text and attach lables
* each buffer has its own coord system, that can be easily accessed
* some utils to align table data
* zellij style tabs and stacks
* icat


## Uart chat pseudo code

```
let inter = Inter::new();
let line_handle = inter.split_below(LinePrompt); // sugar around split(SplitOptions)
start_thread {
  loop {
    let line = line_handle.read_line(); // easy mode for get_string("\n")
    send_line(line);
  }
}

loop {
  let msg = rcv();
  inter.print(msg); // nimmts AsRef<str>, gibts in 2 versionen, eine die den
                    // einfach ausgibt, und eine die nen newline anhaengt
}
```

## Spawn pseudo code

```
loop {
  let line = inter.read_line();

}
```

## Misc

ein gerendertes doc macht sinn, wenn man lines pushen will, nur das anzeigen
was passt, eventuell text wrappen, und scrollen koennen, aber es nimmt einem
dafuer coord access. Also 3 arten splits: doc, raw, input line. In nem doc,
koennte es spaeter nen editor fuer editing geben

terminal apis sind nicht async. und async ist nicht einfach. performance ist
hier nicht der fokus

die new methode muss den initial split type nehmen. die felder brauchen nen
besseren namen

splits haben proportionen. die man bei nem neuen split opitonal mit angeben
koennen sollte.

ausserdem sollte man w und h eines splits querien koennen

man sollte auch bei nem buffer input fns haben, die dann ab dem aktuellen 
cursor funktionieren, und halt mit in den buffer geschrieben werden
