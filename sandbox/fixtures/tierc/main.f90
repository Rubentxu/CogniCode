! Fortran test fixture
program main
    implicit none
    integer :: result
    result = compute(42)
    call greet("world")
    print *, result
contains
    integer function compute(x)
        integer, intent(in) :: x
        compute = x * 2
    end function compute
    subroutine greet(name)
        character(len=*), intent(in) :: name
        print *, "Hello, ", name
    end subroutine greet
end program main
