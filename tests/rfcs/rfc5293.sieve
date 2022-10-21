require ["editheader"];

if not header :contains "X-Sieve-Filtered"
        ["<kim@job.example.com>", "<kim@home.example.com>"]
{
        addheader "X-Sieve-Filtered" "<kim@job.example.com>";
        redirect "kim@home.example.com";
}

deleteheader :index 1 :contains "Delivered-To"
                        "bob@example.com";
addheader "X-Hello" "World";
deleteheader :index 1 "X-Hello";

deleteheader :index 1 :matches "X-Hello" ["hello*world", "hi?there"];

deleteheader :index 1 :is "X-By" "abc";
