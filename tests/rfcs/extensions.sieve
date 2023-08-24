require ["variables", "regex"];

set "A" "1";
set "B" "2";
set "C" "-3";
set "D" "-4.2";
set "E" "5.7";

if string :is "%{A * (B / C) - D + E + global.test}" "1"
{
    keep;
}

set "mixed" "The result of 2 + 3 is %{2 + 3}";
set "mixed2" "The result of 2 + 3 is %{2 + 3} and 3 + 4 is %{3 + 4}";
set "mixed3" "A is ${A} and 2 + 2 is %{2 + 2}!";
set "empty" "%{}";
set "incomplete" "%{2 + 3";

if address :regex :comparator "i;ascii-casemap" "from" [
		"stephan(\\+.*)?@it\\.example\\.com",
		"stephan(\\+.*)?@friep\\.example\\.com"
		] {
    keep;
}
