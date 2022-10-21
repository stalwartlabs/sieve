require ["date", "relational", "fileinto", "index", "vacation", "variables", "editheader"];
if allof(header :is "from" "boss@example.com",
        date :value "ge" :originalzone "date" "hour" "09",
        date :value "lt" :originalzone "date" "hour" "17")
{ fileinto "urgent"; }

if anyof(date :is "received" "weekday" "0",
        date :is "received" "weekday" "6")
{ fileinto "weekend"; }

if anyof(currentdate :is "weekday" "0",
        currentdate :is "weekday" "6",
        currentdate :value "lt" "hour" "09",
        currentdate :value "ge" "hour" "17")
{ redirect "pager@example.com"; }

if allof(currentdate :value "ge" "date" "2007-06-30",
        currentdate :value "le" "date" "2007-07-07")
{ vacation :days 7  "I'm away during the first week in July."; }

if currentdate :matches "month" "*" { set "month" "${1}"; }
if currentdate :matches "year"  "*" { set "year"  "${1}"; }
fileinto "${month}-${year}";

if currentdate :matches "std11" "*"
{addheader "Processing-date" "${0}";}

if date :value "gt" :index 2 :zone "-0500" "received"
        "iso8601" "2007-02-26T09:00:00-05:00"
{ redirect "aftercutoff@example.org"; }

