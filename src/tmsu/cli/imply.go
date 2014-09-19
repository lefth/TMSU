/*
Copyright 2011-2014 Paul Ruane.

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.
*/

package cli

import (
	"fmt"
	"tmsu/common/log"
	"tmsu/storage"
)

var ImplyCommand = Command{
	Name:     "imply",
	Synopsis: "Creates a tag implication",
	Description: `tmsu [OPTION] imply TAG IMPL...
tmsu imply --list

Creates a tag implication such that whenever TAG is applied, IMPL are automatically applied.

It is possible that a file may end up with the same tag applied explicitly and
by way of a tag implication, making the explicit tag redundant. The decision on
whether to keep or remove the redundant explicit tag is with you, but understand
that the implied tags are more flexible in that the rules of which tags implies
which others can be changed at any time.

The 'tags' subcommand can be used to identify which tags applied to a file are
implied.

Examples:

    $ tmsu imply mp3 music
    $ tmsu imply --list
    mp3 ⇒ music
    $ tmsu imply --delete mp3 music`,
	Options: Options{Option{"--delete", "-d", "deletes the tag implication", false, ""},
		Option{"--list", "-l", "lists the tag implications", false, ""}},
	Exec: implyExec,
}

func implyExec(options Options, args []string) error {
	store, err := storage.Open()
	if err != nil {
		return fmt.Errorf("could not open storage: %v", err)
	}
	defer store.Close()

	if err := store.Begin(); err != nil {
		return fmt.Errorf("could not begin transaction: %v", err)
	}
	defer store.Commit()

	switch {
	case options.HasOption("--list"):
		return listImplications(store)
	case options.HasOption("--delete"):
		if len(args) < 2 {
			return fmt.Errorf("implying and implied tag must be specified")
		}

		return deleteImplications(store, args[0], args[1:])
	}

	if len(args) < 2 {
		return fmt.Errorf("implying and implied tags must be specified")
	}

	return addImplications(store, args[0], args[1:])
}

// unexported

func listImplications(store *storage.Storage) error {
	log.Infof(2, "retrieving tag implications.")

	implications, err := store.Implications()
	if err != nil {
		return fmt.Errorf("could not retrieve implications: %v", err)
	}

	width := 0
	for _, implication := range implications {
		length := len(implication.ImplyingTag.Name)
		if length > width {
			width = length
		}
	}

	if len(implications) > 0 {
		previousImplyingTagName := ""
		for _, implication := range implications {
			if implication.ImplyingTag.Name != previousImplyingTagName {
				if previousImplyingTagName != "" {
					fmt.Println()
				}

				previousImplyingTagName = implication.ImplyingTag.Name

				fmt.Printf("%*v => %v", width, implication.ImplyingTag.Name, implication.ImpliedTag.Name)
			} else {
				fmt.Printf(" %v", implication.ImpliedTag.Name)
			}
		}

		fmt.Println()
	}

	return nil
}

func addImplications(store *storage.Storage, tagName string, impliedTagNames []string) error {
	log.Infof(2, "looking up tag '%v'.", tagName)

	tag, err := store.TagByName(tagName)
	if err != nil {
		return fmt.Errorf("could not retrieve tag '%v': %v", tagName, err)
	}
	if tag == nil {
		return fmt.Errorf("no such tag '%v'", tagName)
	}

	for _, impliedTagName := range impliedTagNames {
		log.Infof(2, "looking up tag '%v'", impliedTagName)

		impliedTag, err := store.TagByName(impliedTagName)
		if err != nil {
			return fmt.Errorf("could not retrieve tag '%v': %v", impliedTagName, err)
		}
		if impliedTag == nil {
			return fmt.Errorf("no such tag '%v'", impliedTagName)
		}

		log.Infof(2, "adding tag implication of '%v' to '%v'", tagName, impliedTagName)

		if err = store.AddImplication(tag.Id, impliedTag.Id); err != nil {
			return fmt.Errorf("could not add tag implication of '%v' to '%v': %v", tagName, impliedTagName, err)
		}
	}

	return nil
}

func deleteImplications(store *storage.Storage, tagName string, impliedTagNames []string) error {
	log.Infof(2, "looking up tag '%v'.", tagName)

	tag, err := store.TagByName(tagName)
	if err != nil {
		return fmt.Errorf("could not retrieve tag '%v': %v", tagName, err)
	}
	if tag == nil {
		return fmt.Errorf("no such tag '%v'", tagName)
	}

	for _, impliedTagName := range impliedTagNames {
		log.Infof(2, "looking up tag '%v'.", impliedTagName)

		impliedTag, err := store.TagByName(impliedTagName)
		if err != nil {
			return fmt.Errorf("could not retrieve tag '%v': %v", impliedTagName, err)
		}
		if impliedTag == nil {
			return fmt.Errorf("no such tag '%v'", impliedTagName)
		}

		log.Infof(2, "removing tag implication of '%v' to '%v'.", tagName, impliedTagName)

		if err = store.RemoveImplication(tag.Id, impliedTag.Id); err != nil {
			return fmt.Errorf("could not delete tag implication of '%v' to '%v': %v", tagName, impliedTagName, err)
		}
	}

	return nil
}
