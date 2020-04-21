// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "errors"

func getLabelsByCategory(categoryName string, lf *LabelsFile) ([]Label, error) {
	var labels []Label

	if categoryName == "" {
		return nil, errors.New("need category name")
	}

	for _, label := range lf.Labels {
		if label.CategoryName == categoryName {
			labels = append(labels, label)
		}
	}

	return labels, nil
}
