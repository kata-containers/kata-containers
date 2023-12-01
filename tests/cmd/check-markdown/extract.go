//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	"fmt"

	bf "gopkg.in/russross/blackfriday.v2"
)

// linkDescription extracts the description from the specified link node.
func linkDescription(l *bf.Node) (string, error) {
	if err := checkNode(l, bf.Link); err != nil {
		return "", err
	}

	// A link description can be comprised of various elements so scan
	// through them to build up the final value.

	text := ""
	node := l.FirstChild

	for node != nil {
		switch node.Type {
		case bf.Code:
			text += string(node.Literal)
		case bf.Text:
			text += string(node.Literal)
		default:
			logger.WithField("node", node).Debug("ignoring node")
		}

		if node == l.LastChild {
			break
		}

		node = node.Next
	}

	return text, nil
}

// headingName extracts the heading name from the specified Heading node in
// plain text, and markdown. The latter is used for creating TOC's which need
// to include the original markdown value.
func headingName(h *bf.Node) (name, mdName string, err error) {
	if err = checkNode(h, bf.Heading); err != nil {
		return "", "", err
	}

	// A heading can be comprised of various elements so scan
	// through them to build up the final value.

	node := h.FirstChild

	for node != nil {
		switch node.Type {
		case bf.Code:
			value := string(node.Literal)

			name += value
			mdName += fmt.Sprintf("`%s`", value)
		case bf.Text:
			value := string(node.Literal)

			name += value
			mdName += value
		case bf.Link:
			// yep, people do crazy things like adding links into titles!
			descr, err := linkDescription(node)
			if err != nil {
				return "", "", err
			}

			name += descr
			mdName += descr
		default:
			logger.WithField("node", node).Debug("ignoring node")
		}

		if node == h.LastChild {
			break
		}

		node = node.Next
	}

	return name, mdName, nil
}
