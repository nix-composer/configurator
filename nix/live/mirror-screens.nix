# The live session's screen mirroring (mirror-screens.sh), for the live
# system and its check.
{
  writeShellApplication,
  wlr-randr,
  jq,
}:
writeShellApplication {
  name = "mirror-screens";
  runtimeInputs = [
    wlr-randr
    jq
  ];
  text = builtins.readFile ./mirror-screens.sh;
}
