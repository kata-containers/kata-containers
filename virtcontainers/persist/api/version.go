// Copyright (c) 2019 Huawei Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package persistapi

const (
	// CurPersistVersion is current persist data version.
	// This can help keep backward compatibility, if you make
	// some changes in persistapi package which needs different
	// handling process between different runtime versions, you
	// should modify `CurPersistVersion` and handle persist data
	// according to it.
	// If you can't be sure if the change in persistapi package
	// requires a bump of CurPersistVersion or not, do it for peace!
	// --@WeiZhang555
	CurPersistVersion uint = 2
)
