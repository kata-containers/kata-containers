# SPDX-License-Identifier: Apache-2.0

{
  description = "A portable controller for a Kata Containers test lab";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/ca1659429c950a11a565b5764e37a1b704e17ec8";

  outputs = { nixpkgs, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      eachSystem = nixpkgs.lib.genAttrs systems;
    in {
      devShells = eachSystem (system:
        let
          pkgs = import nixpkgs { inherit system; };
          # Lima discovers virtiofsd through QEMU's vhost-user descriptors.
          qemuForLima = if pkgs.stdenv.isLinux then pkgs.symlinkJoin {
            name = "qemu-lima-virtiofs";
            paths = [ pkgs.qemu pkgs.virtiofsd ];
            postBuild = ''
              mkdir -p "$out/share/qemu/vhost-user"
              cat > "$out/share/qemu/vhost-user/50-lima-virtiofsd.json" <<EOF
              {"type":"fs","binary":"${pkgs.virtiofsd}/bin/virtiofsd"}
              EOF
            '';
          } else pkgs.qemu;
          limaForLab = pkgs.lima.override { qemu = qemuForLima; };
        in {
          default = pkgs.mkShell {
            packages = (with pkgs; [ just git python3 openssh curl shellcheck xorriso oras ])
              ++ [ qemuForLima limaForLab ];
          };
        });
    };
}
