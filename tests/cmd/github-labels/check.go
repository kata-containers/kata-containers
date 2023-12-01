// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"errors"
	"fmt"
	"strings"
	"unicode"
)

func containsWhitespace(s string) bool {
	for _, ch := range s {
		if unicode.IsSpace(ch) {
			return true
		}
	}

	return false
}

func isLower(s string) bool {
	for _, ch := range s {
		if !unicode.IsLetter(ch) {
			continue
		}

		if !unicode.IsLower(ch) {
			return false
		}
	}

	return true
}

func checkCategory(c Category) error {
	if c.Name == "" {
		return fmt.Errorf("category name cannot be blank: %+v", c)
	}

	if containsWhitespace(c.Name) {
		return fmt.Errorf("category name cannot contain whitespace: %+v", c)
	}

	if !isLower(c.Name) {
		return fmt.Errorf("category name must be all lower case: %+v", c)
	}

	if c.Description == "" {
		return fmt.Errorf("category description cannot be blank: %+v", c)
	}

	first := c.Description[0]

	if !unicode.IsUpper(rune(first)) {
		return fmt.Errorf("category description needs initial capital letter: %+v", c)
	}

	if !strings.HasSuffix(c.Description, ".") {
		return fmt.Errorf("category description needs trailing period: %+v", c)
	}

	return nil
}

func checkLabel(l Label) error {
	if l.Name == "" {
		return fmt.Errorf("label name cannot be blank: %+v", l)
	}

	if !isLower(l.Name) {
		return fmt.Errorf("label name must be all lower case: %+v", l)
	}

	if containsWhitespace(l.Name) {
		return fmt.Errorf("label name cannot contain whitespace: %+v", l)
	}

	if l.Description == "" {
		return fmt.Errorf("label description cannot be blank: %+v", l)
	}

	first := l.Description[0]

	if !unicode.IsUpper(rune(first)) {
		return fmt.Errorf("label description needs initial capital letter: %+v", l)
	}

	if l.CategoryName == "" {
		return fmt.Errorf("label category name cannot be blank: %+v", l)
	}

	if l.Colour == "" {
		return fmt.Errorf("label colour cannot be blank: %+v", l)
	}

	return nil
}

func checkLabelsAndCategories(lf *LabelsFile) error {
	catCount := 0

	var catNameMap map[string]int
	var catDescMap map[string]int

	var labelNameMap map[string]int
	var labelDescMap map[string]int

	catNameMap = make(map[string]int)
	catDescMap = make(map[string]int)
	labelNameMap = make(map[string]int)
	labelDescMap = make(map[string]int)

	for _, c := range lf.Categories {
		if err := checkCategory(c); err != nil {
			return err
		}

		catCount++

		if _, ok := catNameMap[c.Name]; ok {
			return fmt.Errorf("duplicate category name: %+v", c)
		}

		catNameMap[c.Name] = 0

		if _, ok := catDescMap[c.Description]; ok {
			return fmt.Errorf("duplicate category description: %+v", c)
		}

		catDescMap[c.Description] = 0
	}

	if catCount == 0 {
		return errors.New("no categories found")
	}

	labelCount := 0

	for _, l := range lf.Labels {
		if err := checkLabel(l); err != nil {
			return err
		}

		if _, ok := labelNameMap[l.Name]; ok {
			return fmt.Errorf("duplicate label name: %+v", l)
		}

		labelNameMap[l.Name] = 0

		if _, ok := labelDescMap[l.Description]; ok {
			return fmt.Errorf("duplicate label description: %+v", l)
		}

		labelDescMap[l.Description] = 0

		labelCount++

		catName := l.CategoryName

		var value int
		var ok bool
		if value, ok = catNameMap[catName]; !ok {
			return fmt.Errorf("invalid category %v found for label %+v", catName, l)
		}

		// Record category name seen and count of occurrences
		value++
		catNameMap[catName] = value
	}

	if labelCount == 0 {
		return errors.New("no labels found")
	}

	if debug {
		fmt.Printf("DEBUG: category count: %v\n", catCount)
		fmt.Printf("DEBUG: label count: %v\n", labelCount)
	}

	for name, count := range catNameMap {
		if count == 0 {
			return fmt.Errorf("category %v not used", name)
		}

		if debug {
			fmt.Printf("DEBUG: category %v: label count: %d\n",
				name, count)
		}
	}

	return nil
}

func check(lf *LabelsFile) error {
	if lf.Description == "" {
		return errors.New("description cannot be blank")
	}

	if lf.Repo == "" {
		return errors.New("repo cannot be blank")
	}

	if len(lf.Categories) == 0 {
		return errors.New("no categories")
	}

	if len(lf.Labels) == 0 {
		return errors.New("no labels")
	}

	return checkLabelsAndCategories(lf)
}
