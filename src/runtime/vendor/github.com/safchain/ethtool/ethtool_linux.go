/*
 *
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *  http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 *
 */

package ethtool

import (
	"golang.org/x/sys/unix"
)

var supportedCapabilities = []struct {
	name  string
	mask  uint64
	speed uint64
}{
	{"10baseT_Half", unix.ETHTOOL_LINK_MODE_10baseT_Half_BIT, 10_000_000},
	{"10baseT_Full", unix.ETHTOOL_LINK_MODE_10baseT_Full_BIT, 10_000_000},
	{"100baseT_Half", unix.ETHTOOL_LINK_MODE_100baseT_Half_BIT, 100_000_000},
	{"100baseT_Full", unix.ETHTOOL_LINK_MODE_100baseT_Full_BIT, 100_000_000},
	{"1000baseT_Half", unix.ETHTOOL_LINK_MODE_1000baseT_Half_BIT, 1_000_000_000},
	{"1000baseT_Full", unix.ETHTOOL_LINK_MODE_1000baseT_Full_BIT, 1_000_000_000},
	{"10000baseT_Full", unix.ETHTOOL_LINK_MODE_10000baseT_Full_BIT, 10_000_000_000},
	{"2500baseT_Full", unix.ETHTOOL_LINK_MODE_2500baseT_Full_BIT, 2_500_000_000},
	{"1000baseKX_Full", unix.ETHTOOL_LINK_MODE_1000baseKX_Full_BIT, 1_000_000_000},
	{"10000baseKX_Full", unix.ETHTOOL_LINK_MODE_10000baseKX4_Full_BIT, 10_000_000_000},
	{"10000baseKR_Full", unix.ETHTOOL_LINK_MODE_10000baseKR_Full_BIT, 10_000_000_000},
	{"10000baseR_FEC", unix.ETHTOOL_LINK_MODE_10000baseR_FEC_BIT, 10_000_000_000},
	{"20000baseMLD2_Full", unix.ETHTOOL_LINK_MODE_20000baseMLD2_Full_BIT, 20_000_000_000},
	{"20000baseKR2_Full", unix.ETHTOOL_LINK_MODE_20000baseKR2_Full_BIT, 20_000_000_000},
	{"40000baseKR4_Full", unix.ETHTOOL_LINK_MODE_40000baseKR4_Full_BIT, 40_000_000_000},
	{"40000baseCR4_Full", unix.ETHTOOL_LINK_MODE_40000baseCR4_Full_BIT, 40_000_000_000},
	{"40000baseSR4_Full", unix.ETHTOOL_LINK_MODE_40000baseSR4_Full_BIT, 40_000_000_000},
	{"40000baseLR4_Full", unix.ETHTOOL_LINK_MODE_40000baseLR4_Full_BIT, 40_000_000_000},
	{"56000baseKR4_Full", unix.ETHTOOL_LINK_MODE_56000baseKR4_Full_BIT, 56_000_000_000},
	{"56000baseCR4_Full", unix.ETHTOOL_LINK_MODE_56000baseCR4_Full_BIT, 56_000_000_000},
	{"56000baseSR4_Full", unix.ETHTOOL_LINK_MODE_56000baseSR4_Full_BIT, 56_000_000_000},
	{"56000baseLR4_Full", unix.ETHTOOL_LINK_MODE_56000baseLR4_Full_BIT, 56_000_000_000},
	{"25000baseCR_Full", unix.ETHTOOL_LINK_MODE_25000baseCR_Full_BIT, 25_000_000_000},
}
