// Clean: Using generic types
import java.util.List;
import java.util.ArrayList;

public class RawTypeExample {
    public List<String> getList() {
        List<String> list = new ArrayList<>();
        list.add("item");
        return list;
    }
}
