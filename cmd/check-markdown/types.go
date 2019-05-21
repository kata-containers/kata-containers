//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import "github.com/Sirupsen/logrus"

// LinkType represents the type of a link in a markdown document.
type LinkType int

const (
	unknownLink   LinkType = iota
	internalLink  LinkType = iota
	externalLink  LinkType = iota // External ".md" file
	externalFile  LinkType = iota // External non-".md" file
	urlLink       LinkType = iota
	mailLink      LinkType = iota
	LinkTypeCount LinkType = iota
)

func (t LinkType) String() string {
	var name string

	switch t {
	case unknownLink:
		name = "unknown"
	case internalLink:
		name = "internal-link"
	case externalLink:
		name = "external-link"
	case externalFile:
		name = "external-file"
	case urlLink:
		name = "url-link"
	case mailLink:
		name = "mail-link"
	}

	return name
}

// Heading is a markdown heading, which might be the destination
// for a link.
//
// Example: A heading like this:
//
//    ### This is a heading
//
// ... would be described as:
//
// ```go
// Heading{
//   Name:     "This is a heading",
//   LinkName: "this-is-a-heading",
//   Level:    3,
// }
// ```
type Heading struct {
	// Not strictly necessary since the name is used as a hash key.
	// However, storing here too makes the code simpler ;)
	Name string

	// The encoded value of Name.
	LinkName string

	// Heading level (1 for top level)
	Level int
}

// Link is a reference to another part of this document
// (or another document).
//
// Example: A link like this:
//
//     [internal link](#internal-section-name)
//
// ... would be described as:
//
// ```go
// Link{
//   Address:     "internal-section-name",
//   Description: "internal link",
//   Type:        internalLink,
// }
// ```
type Link struct {
	// Must be a valid Heading.LinkName.
	//
	// Not strictly necessary since the address is used as a hash key.
	// However, storing here too makes the code simpler ;)
	Address string

	// The text the user sees for the hyperlink address
	Description string

	Type LinkType
}

// Doc represents a markdown document.
type Doc struct {
	// Filename
	Name string

	// Key: heading name
	// Value: Heading
	Headings map[string]Heading

	// Key: link address
	// Value: *list* of links. Required since you can have multiple links with
	// the same _address_, but of a different type.
	Links map[string][]Link

	// true when this document has been fully parsed
	Parsed bool

	// if true, only show the Table Of Contents
	ShowTOC bool

	Logger *logrus.Entry
}
