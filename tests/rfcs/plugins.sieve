require ["vnd.stalwart.expressions"];

plugin1 :tag1 :tag2 :string_arg "test" "8.3" ["a", "b", "c"];

if plugin1 :string_arg "test2" "1.3" "x" {
    keep;
}

plugin2 :array_arg ["1", "2", "3"] [".*\\.uk", ".*\\.nl", ".*\\.tk"];

if plugin2 :array_arg "10" "the (.*) jumps off the (.*)" {
    keep;
}

if anyof(plugin1 :tag1 :tag2 :string_arg "test" "8.3" ["a", "b", "c"], plugin2 :array_arg ["1", "2", "3"] [".*\\.uk", ".*\\.nl", ".*\\.tk"]) {
    discard;
}

