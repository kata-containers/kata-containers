//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"regexp"
	"strings"

	bf "gopkg.in/russross/blackfriday.v2"
)

// handleNode processes the specified node.
func (d *Doc) handleNode(node *bf.Node) error {
	var err error

	switch node.Type {
	case bf.Heading:
		err = d.handleHeading(node)
	case bf.Link:
		err = d.handleLink(node)
	case bf.Text:
		// handle blackfriday deficiencies
		headings, err := d.forceCreateHeadings(node)
		if err != nil {
			return err
		}

		for _, heading := range headings {
			err := d.addHeading(heading)
			if err != nil {
				return err
			}
		}

	default:
		return nil
	}

	return err
}

// makeHeading creates a heading from the specified node.
func (d *Doc) makeHeading(node *bf.Node) (Heading, error) {
	if err := checkNode(node, bf.Heading); err != nil {
		return Heading{}, err
	}

	name, err := headingName(node)
	if err != nil {
		return Heading{}, d.Errorf("failed to get heading name: %v", err)
	}

	data := node.HeadingData

	heading := Heading{
		Name:     name,
		LinkName: data.HeadingID,
		Level:    data.Level,
	}

	return heading, nil
}

// handleHeading processes the heading represented by the specified node.
func (d *Doc) handleHeading(node *bf.Node) error {
	if err := checkNode(node, bf.Heading); err != nil {
		return err
	}

	heading, err := d.makeHeading(node)
	if err != nil {
		return err
	}

	return d.addHeading(heading)
}

func (d *Doc) handleLink(node *bf.Node) error {
	if err := checkNode(node, bf.Link); err != nil {
		return err
	}

	address := string(node.Destination)

	description, err := linkDescription(node)
	if err != nil {
		return d.Errorf("failed to get link name: %v", err)
	}

	link := Link{
		Address:     address,
		Description: description,
	}
	// markdown file extension with optional link name ("#...")
	const re = `\.md#*.*$`

	pattern := regexp.MustCompile(re)

	matched := pattern.MatchString(address)

	if strings.HasPrefix(address, "http") {
		link.Type = urlLink
	} else if strings.HasPrefix(address, "mailto:") {
		link.Type = mailLink
	} else if strings.HasPrefix(address, anchorPrefix) {
		link.Type = internalLink

		// Remove the prefix to make a valid link address
		address = strings.TrimPrefix(address, anchorPrefix)
		link.Address = address

	} else if matched {
		link.Type = externalLink
	} else {
		// Link must be an external file, but not a markdown file.
		link.Type = externalFile
	}

	return d.addLink(link)
}

// handleIntraDocLinks checks the links between documents are correct.
//
// For example, if a document refers to "foo.md#section-bar", this function
// will ensure that "section-bar" exists in external file "foo.md".
func handleIntraDocLinks() error {
	for _, doc := range docs {
		for addr, linkList := range doc.Links {
			for _, link := range linkList {
				err := doc.checkLink(addr, link, true)
				if err != nil {
					return doc.Errorf("intra-doc link invalid: %v", err)
				}
			}
		}
	}

	return nil
}
