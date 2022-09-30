require "environment";

if environment :matches "remote-host" "*.example.com" { discard; }