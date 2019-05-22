// Copyright (c) 2017-2018 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

type CfgOBSProject struct {
	Name string
	Auth struct {
		User     string
		Password string
	}
	Releases []string
	Archs    []string `yaml:"architectures"`
}

type CfgPackagecloud struct {
	Auth struct {
		User  string
		Token string
	}
	Repo string
}

type config struct {
	OBSProjects  map[string]CfgOBSProject `yaml:"obsprojects"`
	Packagecloud CfgPackagecloud
	// Mapping from OBS "Repositories" to Packagecloud "Distros"
	DistroMapping map[string]string `yaml:"distro-mapping"`
}

func getConfig(configFile string) (config, error) {
	var cfg config
	if err := yamlUnmarshal(configFile, &cfg); err != nil {
		return cfg, err
	}
	return cfg, nil
}
