package docker

import (
	"io/ioutil"
	"os"

	. "github.com/onsi/ginkgo"
	. "github.com/onsi/gomega"
)

var _ = Describe("load", func() {
	var (
		id        string
		imageName string
		exitCode  int
	)

	BeforeEach(func() {
		id = randomDockerName()
		_, _, exitCode = dockerRun("-td", "--name", id, Image)
		Expect(exitCode).To(Equal(0))
	})

	AfterEach(func() {
		_, _, exitCode = dockerRmi(imageName)
		Expect(exitCode).To(Equal(0))
		Expect(RemoveDockerContainer(id)).To(BeTrue())
		Expect(ExistDockerContainer(id)).NotTo(BeTrue())
	})

	Describe("load with docker", func() {
		Context("load a container", func() {
			It("should load image", func() {
				file, err := ioutil.TempFile(os.TempDir(), "mynewimage.tar")
				Expect(err).ToNot(HaveOccurred())
				err = file.Close()
				Expect(err).ToNot(HaveOccurred())
				defer os.Remove(file.Name())
				Expect(file.Name()).To(BeAnExistingFile())
				imageName = "test/container-test"
				_, _, exitCode = dockerCommit(id, imageName)
				Expect(exitCode).To(Equal(0))
				_, _, exitCode = dockerSave(imageName, "--output", file.Name())
				Expect(exitCode).To(Equal(0))
				stdout, _, _ := dockerLoad("--input", file.Name())
				Expect(stdout).To(ContainSubstring(imageName))
			})
		})
	})
})
