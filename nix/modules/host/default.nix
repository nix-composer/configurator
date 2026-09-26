# What generated host flakes import from the configurator
# (`inputs.configurator.nixosModules.default`): the tasks the install
# leaves for later.
{ ... }:
{
  imports = [ ./tpm-pin.nix ];
}
