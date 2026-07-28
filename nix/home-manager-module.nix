{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.programs.dispatch;
  settingsFormat = pkgs.formats.toml { };
  configFile = settingsFormat.generate "config.toml" cfg.settings;
in {
  meta.maintainers = [ "christo-auer" ];

  options.programs.dispatch = {
    enable = mkEnableOption "dispatch, a feature-rich TUI RSS reader";

    package = mkOption {
      type = types.package;
      default = pkgs.dispatch;
      defaultText = literalExpression "pkgs.dispatch";
      description = "The dispatch package to use.";
    };

    settings = mkOption {
      type = settingsFormat.type;
      default = { };
      example = literalExpression ''
        {
          refresh_fps = 60;
          article_scope = "unread";
          read_icon = "󰄬";
          unread_icon = "󰄱";
          
          theme = {
            color_palette = {
              background = "#1e1e2e";
              foreground = "#cdd6f4";
              accent_primary = "#f5c2e7";
            };
          };
          
          input_config = {
            scroll_amount = 10;
            mappings = {
              "q" = "quit";
              "j" = "down";
              "k" = "up";
            };
          };
        }
      '';
      description = ''
        Configuration written to {file}`$XDG_CONFIG_HOME/dispatch/config.toml`.
        
        See <https://github.com/christo-auer/dispatch#configuration-options>
        for the full list of options.
      '';
    };
  };

  config = mkIf cfg.enable {
    home.packages = [ cfg.package ];

    xdg.configFile."dispatch/config.toml" = mkIf (cfg.settings != { }) {
      source = configFile;
    };
  };
}
