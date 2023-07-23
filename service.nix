{ outputs, system }:
{
  lib,
  config,
  ...
}: let
  cfg = config.services.lumin;
  pkg = outputs.defaultPackage.${system};
in {
  options.services.lumin = {
    enable = lib.mkEnableOption "enable the lumin service";

    site = lib.mkOption {
      description = "path to the site";
      type = lib.types.path;
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.lumin = {
      description = "my blog site";
      wantedBy = ["multi-user.target"];

      serviceConfig = {
        ExecStart = "${pkg}/bin/lumin ${cfg.site}";
        ProtectHome = "read-only";
        Restart = "on-failure";
        Type = "exec";
        DynamicUser = true;
      };
    };
  };
}
